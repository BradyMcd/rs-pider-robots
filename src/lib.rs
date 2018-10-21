//
// TODO: Getters, Casing anomalies, Tests, Documentation, HACK comments

extern crate reqwest;
extern crate base_url;
extern crate try_from;

use std::convert::*;

use reqwest::{ Response };

use try_from::TryFrom;
use base_url::BaseUrl;

/// A set of observed anomalies in the robots.txt file
/// Anything not directly interacted with through the rest of this api is considered anomalous, and
/// includes comments and illegal (cross host) rule lines as well as unknown or unimplemented
/// directives.
pub enum Anomaly {
    /// Any comment stored alongside some context, either the rest of the line the comment was found on
    /// or the line following. Context strings may be observed twice if a block comment is placed above
    /// a line with a directive and comment.
    Comment( String /*The comment*/, String /*The context*/ ),
    /// Rules whose names are not in the normal casing format, ie. "allow" rather than "Allow"
    Casing( String, BaseUrl ), // mostly here to guage if this type of error is common
    /// A Rule located outside of a User-agent section
    OrphanRule( Rule ),
    /// A User-agent line nested in another User-agent section which already contains one or more Rules
    RecursedUserAgent( String /*The agent's name*/ ),
    /// Any known directive not noramlly found in a User-agent section.
    MissSectionedDirective( String, String ),
    /// Any directive which is unimplemented or otherwise unknown
    UnknownDirective( String, String ),
    /// Any line which isn't in the standard format for a robots.txt file, ie. a line without a ':'
    /// separator which is not a comment
    UnknownFormat( String ),
}

pub enum Rule { //TODO: consider rolling a Path type to deal with the BaseUrl to String interface
    AllowAll( String ),
    DisallowAll( String ),
    Allow( String ),
    Disallow( String ),
    /* TODO:
     * Crawl-delay
     * Request-rate
     */

}

pub struct UserAgent {
    names: Vec< String >,
    rules: Vec< Rule >,
    anomalies: Vec< Anomaly >,
}

pub struct RobotsParser {
    host: BaseUrl,
    sitemaps: Vec<BaseUrl>,
    agents: Vec<UserAgent>,
    anomalies: Vec< Anomaly >,
}

enum R_State { //Recursed state; useragent sections don't recurse, they add
    Comment( UserAgent, String ),
    Normal( UserAgent ),
}

enum State {
    Comment( RobotsParser, String ), //We have a comment, but we can't see any context yet
    Agent( RobotsParser, R_State ), //We are inside of a useragent section
    Normal( RobotsParser ), //Any lines at the root level (those without a useragent association)
}

enum DirectiveResult<'a> {
    Ok_UserAgent( String ),
    Ok_Rule( Rule ),
    Ok_Sitemap( &'a str ),
    //Ok_RequestRate( u32 ),
    //Ok_CrawlDelay( u32 ),
    Unknown(),
}

fn parse_directive<'a>( directive: &str, argument: &'a str ) -> DirectiveResult< 'a > {
    match directive {
        "User-agent" => {
            DirectiveResult::Ok_UserAgent( argument.to_string( ) )
        }
        "Disallow" => {
            DirectiveResult::Ok_Rule( Rule::new( false, argument.to_string( ) ) )
        }
        "Allow" => {
            DirectiveResult::Ok_Rule( Rule::new( true, argument.to_string( ) ) )
        }
        "Sitemap" => {
            DirectiveResult::Ok_Sitemap( argument )
        }
        _ => {
            DirectiveResult::Unknown()
        }
    }
}

impl Rule {

    fn new( allowance: bool, path: String ) -> Rule {
        match allowance {
            true => {
                if path == "*" {
                    Rule::AllowAll( path )
                } else {
                    Rule::Allow( path )
                }
            }
            false => {
                if path == "*" {
                    Rule::DisallowAll( path )
                } else {
                    Rule::Disallow( path )
                }
            }
        }
    }

}

impl UserAgent {

    fn new( agent: String ) -> Self {
        UserAgent{
            names: vec!( agent.to_string( ) ),
            rules: Vec::new( ),
            anomalies: Vec::new( ),
        }
    }

    fn is_empty( &self ) -> bool {
        self.rules.is_empty( )
    }

    fn add_agent( &mut self, name: String ) {
        if self.is_empty( ) {
            self.names.push( name );
        } else {
            self.anomalies.push( Anomaly::RecursedUserAgent( name ) );
        }
    }

    fn add_rule( &mut self, rule: Rule ) {
        self.rules.push( rule );
    }

    fn add_comment( &mut self, context: String, comment: String ) {
        self.anomalies.push( Anomaly::Comment( comment, context ) );
    }

    fn add_anomaly( &mut self, anomaly: Anomaly ) {
        self.anomalies.push( anomaly );
    }
}

impl R_State {

    fn empty_line( self ) -> UserAgent {

        match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( "".to_string( ), s );
                u
            },
            R_State::Normal( u ) => {
                u
            },
        }
    }

    fn comment( self, line: &str ) -> Self {

        match self {
            R_State::Comment( u, mut s ) => {
                s.push_str( line );
                R_State::Comment( u, s )
            },
            R_State::Normal( u ) => R_State::Comment( u, String::from( line ) ),
        }
    }

    fn context_comment( self, context: &str, comment: &str ) -> Self {

        match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( s.to_string( ), context.to_string( ) );
                u.add_comment( comment.to_string( ), context.to_string( ) );
                R_State::Normal( u )
            }
            R_State::Normal( mut u ) => {
                u.add_comment( comment.to_string( ), context.to_string( ) );
                R_State::Normal( u )
            }
        }
    }

    fn directive_line( self, directive: &str, argument: &str ) -> Self {

        let mut user_agent;

        match self {
            R_State::Comment( mut u, s ) => {
                let mut context = format!( "{}: {}", directive, argument );
                u.add_comment( context, s.to_string( ) );
                user_agent = u;
            }
            R_State::Normal( u ) => {
                user_agent = u;
            }
        }

        match parse_directive( directive, argument ) {
            DirectiveResult::Ok_UserAgent( ua ) => {
                user_agent.add_agent( ua );
                R_State::Normal( user_agent )
            }
            DirectiveResult::Ok_Rule( r ) => {
                user_agent.add_rule( r );
                R_State::Normal( user_agent )
            }
            DirectiveResult::Unknown() => {
                user_agent.add_anomaly(
                    Anomaly::UnknownDirective( directive.to_string( ),
                                               argument.to_string( ) )
                );
                R_State::Normal( user_agent )
            }
            _ => {
                user_agent.add_anomaly(
                    Anomaly::MissSectionedDirective( directive.to_string( ), argument.to_string( ) )
                );
                R_State::Normal( user_agent )
            }
        }
    }

    fn anomaly( self, line: &str ) -> Self {

        match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( s.to_string( ), line.to_string( ) );
                R_State::Normal( u )
            }
            R_State::Normal( mut u ) => {
                u.add_anomaly( Anomaly::UnknownFormat( line.to_string( ) ) );
                R_State::Normal( u )
            }
        }
    }
}

impl State {

    fn empty_line( self ) -> Self {

        match self {
            State::Comment( r, s ) => {
                State::Comment( r, s )
            }
            State::Agent( mut r, s ) => {
                r.add_agent( s.empty_line( ) );
                State::Normal( r )
            }
            State::Normal( r ) => {
                State::Normal( r )
            }
        }
    }

    fn comment( self, line: &str ) -> Self {

        match self {
            State::Comment( r, mut s ) => {
                s.push_str( line );
                State::Comment( r, s )
            },
            State::Agent( r, mut s ) => {
                s = s.comment( line );
                State::Agent( r, s )
            },
            State::Normal( r ) => State::Comment( r, String::from( line ) ),
        }
    }

    fn context_comment( self, context: &str, comment: &str ) -> Self {

        match self {
            State::Comment( mut r, s ) =>{
                r.add_comment( context.to_string( ), s.to_string( ) );
                r.add_comment( context.to_string( ), comment.to_string( ) );
                State::Normal( r )
            }
            State::Agent( r, mut s ) => {
                s = s.context_comment( context, comment );
                State::Agent( r, s )
            }
            State::Normal( mut r ) => {
                r.add_comment( context.to_string( ), comment.to_string( ) );
                State::Normal( r )
            }
        }
    }

    fn directive_line( self, directive: &str , argument: &str ) -> Self {

        let mut robots;

        match self {
            State::Comment( mut r, s ) => {
                let mut context = format!( "{}: {}", directive, argument );
                r.add_comment( context, s.to_string( ) );
                robots = r;
            }
            State::Agent( r, mut s ) => {
                s = s.directive_line( directive, argument );
                return State::Agent( r, s );
            }
            State::Normal( r ) => {
                robots = r;
            }
        }

        match parse_directive( directive, argument ) {
            DirectiveResult::Ok_UserAgent( ua ) => {
                State::Agent( robots, R_State::Normal( UserAgent::new( ua ) ) )
            }
            DirectiveResult::Ok_Rule( r ) => {
                robots.add_anomaly( Anomaly::OrphanRule( r ) );
                State::Normal( robots )
            }
            DirectiveResult::Ok_Sitemap( s ) => {
                let mut sitemap_url = robots.host_url( );
                sitemap_url.set_path( s );
                robots.add_sitemap( sitemap_url );
                State::Normal( robots )
            }
//            DirectiveResult::Ok_RequestRate( r ) => {}
//            DirectiveResult::Ok_CrawlDelay( d ) = > {}
            DirectiveResult::Unknown() => {
                robots.add_anomaly(
                    Anomaly::UnknownDirective( directive.to_string( ),
                                               argument.to_string( ) )
                );
                State::Normal( robots )
            }
        }
    }

    fn anomaly( self, line: &str ) -> Self {
        match self {
            State::Comment( mut r, s ) => {
                r.add_comment( line.to_string( ), s.to_string( ) );
                r.add_unknown( s.to_string( ) );
                State::Normal( r )
            }
            State::Agent( r, mut s ) => {
                s = s.anomaly( line );
                State::Agent( r, s )
            }
            State::Normal( mut r ) => {
                r.add_unknown( line.to_string( ) );
                State::Normal( r )
            }
        }
    }

    fn eof( self ) -> RobotsParser {
        match self {
            State::Comment( mut r, s ) => {
                r.add_comment( "[EOF]".to_string( ), s.to_string( ) );
                r
            }
            State::Agent( mut r, s ) => {
                r.add_agent( s.empty_line( ) );
                r
            }
            State::Normal( r ) => {
                r
            }
        }

    }
}

impl RobotsParser {

    /***********
     * Private methods
     ******/

    fn add_comment( &mut self, context: String, comment: String ) {
        self.anomalies.push( Anomaly::Comment( comment, context ) );
    }

    fn add_agent( &mut self, agent: UserAgent ) {
        self.agents.push( agent );
    }

    fn add_sitemap( &mut self, url: BaseUrl ) {
        self.sitemaps.push( url );
    }

    fn add_anomaly( &mut self, anomaly: Anomaly ) {
        self.anomalies.push( anomaly );
    }

    fn add_unknown( &mut self, line: String ) {
        self.anomalies.push( Anomaly::UnknownFormat( line ) );
    }

    /***********
     * Creation
     ******/

    pub fn guess_robots_url( &self ) -> BaseUrl {
        let mut ret = self.host.clone( );
        ret.set_path( "/robots.txt" );
        return ret;
    }

    pub fn from_response( mut response: Response ) -> RobotsParser {
        assert!( response.status( ).is_success( ) );

        //NOTE: brittle
        let mut host = match BaseUrl::try_from( response.url( ).clone( ) ) {
            Ok( u ) => u,
            Err( _e ) => panic!( ),
        };
        host.set_path( "/" );

        let text = match response.text( ) {
            Ok( t ) => t,
            Err( _e ) => panic!( ),
        };

        Self::parse( host, text )
    }

    pub fn parse< S: Into<String> >( host: BaseUrl, text: S ) -> RobotsParser {
        let text = text.into( ); //Not a one-liner to appease lifetimes
        let lines = text.lines( );
        let ret = RobotsParser{
            host: host,
            sitemaps: Vec::new( ),
            agents: Vec::new( ),
            anomalies: Vec::new( ),
        };

        let mut state = State::Normal( ret );

        for _line in lines {
            //NOTE: in both of the split_at directives the split character goes into r
            let mut line = _line.trim( ); //clear any whitespace

            /***********
             * Empty Lines
             ******/
            if line.is_empty( ) {
                state = state.empty_line( );
                continue;
            }

            /***********
             * Comments
             ******/
            if line.starts_with( "#" ) {
                state = state.comment( line );
                continue;
            } else if line.contains( "#" ) {
                let ( l, r ) = line.split_at( line.find( "#" ).unwrap( ) );
                state = state.context_comment( l, r );
                line = l.trim( );
            }

            /***********
             * Directives
             ******/
            if line.contains( ":" ) {
                let ( l, r ) = line.split_at( line.find( ":" ).unwrap( ) );
                state = state.directive_line( l, r.trim_left_matches( ':' ) );
            } else {
                /***********
                 * Everything else
                 ******/
                state = state.anomaly( line );
            }
        }

        state.eof( )
    }

    /***********
     * Getters
     ******/

    pub fn host_url( &self ) -> BaseUrl {
        self.host.clone( )
    }
}


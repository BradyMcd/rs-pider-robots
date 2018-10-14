//


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
    /// or the line following
    Comment( String /*The comment*/, String /*The context*/ ),
    /// Any rule which refers to a remote host to the robots.txt file
    CrossHostRule( String, BaseUrl ),
    /// Rules whose names are not in the normal casing format, ie. "allow" rather than "Allow"
    Casing( String, BaseUrl ), // mostly here to guage if this type of error is common
    /// A known directive associated with a User-agent not normally associated as such or vice versa.
    /// ie. a Disallow rule outside of a User-agent or a Sitemap inside of one.
    MissSectioned( String, BaseUrl ),
    /// Any directive which is unimplemented or otherwise unknown
    UnknownDirective( String, Option< String > ),
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

impl UserAgent {
    fn add_comment( &mut self, context: String, comment: String ) {
        self.anomalies.push( Anomaly::Comment( comment, context ) );
    }

    fn add_anomaly( &mut self, line: String ) {
        self.anomalies.push( Anomaly::UnknownFormat( line ) );
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

    fn anomaly( self, line: &str ) -> Self {

        match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( s.to_string( ), line.to_string( ) );
                R_State::Normal( u )
            }
            R_State::Normal( mut u ) => {
                u.add_anomaly( line.to_string( ) );
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
            State::Normal( mut r ) => {
                robots = r;
            }
        }

        //Now match the directive with known directives
        match directive {
            "User-agent" => {
                return State::Agent(
                    robots,
                    R_State::Normal( UserAgent::new( argument ) ),
                );
            }
            "Disallow" => {
                robots.anomalies.push( Anomaly::MissSectioned(
                    directive.to_string( ),
                    robots.host_url( ).set_path( argument ),
                ) );
            }
            "Allow" => {}
            "Sitemap" => {}
        }

        State::Normal( robots )
    }

    fn anomaly( self, line: &str ) -> Self {
        match self {
            State::Comment( mut r, s ) => {
                r.add_comment( line.to_string( ), s.to_string( ) );
                r.add_anomaly( s.to_string( ) );
                State::Normal( r )
            }
            State::Agent( r, mut s ) => {
                s = s.anomaly( line );
                State::Agent( r, s )
            }
            State::Normal( mut r ) => {
                r.add_anomaly( line.to_string( ) );
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

    fn add_anomaly( &mut self, line: String ) {
        self.anomalies.push( Anomaly::UnknownFormat( line ) );
    }

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
}


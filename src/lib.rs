//
// TODO: Getters, Tests, Documentation/Code Rearrangement, HACK comments
// TODO: RE: Code Rearrangement: Isolate the parsing logic from the main structure
// TODO: Sort rule entries by order of specificity, that means User-agent sections go wildcard first as do rules
extern crate reqwest;
extern crate base_url;
extern crate try_from;

use std::convert::*;
use std::cmp::Ordering;

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
    /// Rules whose names are not in the normal casing format, ie. "foo" rather than "Foo"
    Casing( String, String ), // mostly here to guage if this type of error is common
    /// A Rule located outside of a User-agent section
    OrphanRule( Rule ),
    /// A User-agent line nested in another User-agent section which already contains one or more Rules
    RecursedUserAgent( String /*The agent's name*/ ),
    /// A User-agent which contains both a wildcard and a specific User-agent name
    RedundantWildcardUserAgent( String ),
    /// Any known directive not noramlly found in a User-agent section
    MissSectionedDirective( String, String ),
    /// Any directive which is unimplemented or otherwise unknown
    UnknownDirective( String, String ),
    /// Any line which isn't in the standard format for a robots.txt file, ie. a line without a ':'
    /// separator which is not a comment
    UnknownFormat( String ),
}

/// Represents a Rule line found in a User-agent section
#[derive( Clone, PartialEq, Eq )]
pub enum Rule {
    Allow( String ),
    Disallow( String ),
    /* TODO:
     * Crawl-delay
     * Request-rate
     */

}

/// A User-agent section and all names, rules and anomalies associated
pub struct UserAgent {
    names: Vec< String >,
    rules: Vec< Rule >,
    anomalies: Vec< Anomaly >,
}

/// Represents a parsed robots.txt file
pub struct RobotsParser {
    host: BaseUrl,
    sitemaps: Vec<BaseUrl>,
    agents: Vec<UserAgent>,
    anomalies: Vec< Anomaly >,
}

//--SNIP--
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
                Rule::Allow( path )
            }
            false => {
                Rule::Disallow( path )
            }
        }
    }

    fn applies( &self, url: &BaseUrl ) -> bool {
        let url_specificity = Self::path_specificity( url.path( ) );
        let self_specificity;
        let url_path = url.path( ).split( '/' );
        let self_path = match self {
            Rule::Allow( path ) | Rule::Disallow( path ) => {
                if path == "*" { return true; }
                self_specificity = Self::path_specificity( path );
                path.split( '/' )
            }
        };

        if url_specificity < self_specificity {
            false
        } else {
            for segments in url_path.zip( self_path ) {
                let ( url_seg, self_seg ) = segments;
                if url_seg != self_seg && !url_seg.is_empty( ) {
                    return false;
                }
            }
            true
        }
    }

    /***********
     * Ordering Helpers
     ******/

    fn is_allow( &self ) -> bool {
        match self {
            Rule::Allow( _ ) => { true }
            _ => { false }
        }
    }

    fn path_specificity( path: &str ) -> usize {
        path.split( '/' ).filter( | segment |{ !segment.is_empty( ) } ).count( )
    }

    fn specificity( &self ) -> usize {
        match self {
            Rule::Allow( path ) | Rule::Disallow( path ) => {
                Self::path_specificity( path )
            }
        }
    }

}

/// Less specific rules are considered to be "greater" than more specific counterparts so that they
/// will be considered first by the permissions logic. The more path segments a Rule contains the
/// more specific it is. Allow rules are also considered greater than Disallow rules all other
/// things being equal. Rules considered "least" are handled last by the permissions logic and so
/// "overwrite" earlier rules.
impl PartialOrd for Rule {

    fn partial_cmp( &self, rhs: &Self ) -> Option< Ordering > {
        let right_spec = rhs.specificity( );
        let left_spec = self.specificity( );

        if left_spec == right_spec {
            return Some (
                if self.is_allow( ) == rhs.is_allow( ) {
                    Ordering::Equal
                } else if self.is_allow( ) {
                    Ordering::Greater
                } else {
                    Ordering::Less
                } )
        } else {
            Some( left_spec.cmp( &right_spec ) )
        }
    }
}

impl Ord for Rule {
    fn cmp( &self, rhs: &Self ) -> Ordering {
        self.partial_cmp( rhs ).unwrap( )
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

        if name == "*" || self.names.contains( &String::from( "*" ) ) {
            self.anomalies.push( Anomaly::RedundantWildcardUserAgent( name.clone( ) ) );
        }

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

impl PartialEq for UserAgent {
    fn eq( &self, rhs:&Self ) -> bool {
        self.names == rhs.names
    }
}
impl Eq for UserAgent {}

// I (vaguely) wonder if there is a builtin which has this effect
fn reverse_ord( order: Ordering ) -> Ordering {
    match order {
        Ordering::Greater => {
            Ordering::Less
        }
        Ordering::Less => {
            Ordering::Greater
        }
        _ => order
    }
}

/// Less specific User-agents are considered greater than more specific User-agents since the
/// permissions logic ought to consider specific directives as overwriting general ones. That means
/// User-agent sections containing wildcards are greatest and thereafter are sorted by the number of
/// specific names which their Rule entries apply to.
impl PartialOrd for UserAgent {
    fn partial_cmp( &self, rhs: &Self ) -> Option< Ordering > {
        let wildcard = String::from( "*" );
        if self.names.contains( &wildcard ) {
            Some( Ordering::Greater )
        } else if rhs.names.contains( &wildcard ) {
            Some( Ordering::Less )
        }else {
            Some( reverse_ord( self.names.len( ).cmp( &rhs.names.len( ) ) ) )
        }
    }
}

impl Ord for UserAgent {
    fn cmp( &self, rhs: &Self ) -> Ordering {
        self.partial_cmp( rhs ).unwrap( )
    }
}

impl R_State {

    fn empty_line( self ) -> UserAgent {

        let mut ret = match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( "".to_string( ), s );
                u
            },
            R_State::Normal( u ) => {
                u
            },
        };
        ret.rules.sort( );
        ret
    }

    fn comment( self, line: &str ) -> Self {

        match self {
            R_State::Comment( u, mut s ) => {
                s.push_str( "\n" );
                s.push_str( line );
                R_State::Comment( u, s )
            },
            R_State::Normal( u ) => R_State::Comment( u, String::from( line ) ),
        }
    }

    fn context_comment( self, context: &str, comment: &str ) -> Self {

        match self {
            R_State::Comment( mut u, s ) => {
                u.add_comment( context.to_string( ), s.to_string( ) );
                u.add_comment( context.to_string( ), comment.to_string( ) );
                R_State::Normal( u )
            }
            R_State::Normal( mut u ) => {
                u.add_comment( context.to_string( ), comment.to_string( ) );
                R_State::Normal( u )
            }
        }
    }

    fn directive_line( self, mut directive: String, argument: String ) -> Self {

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

        //HACK: Space saving is possible by moving the Casing anomaly check
        if directive.starts_with( | c: char |( c.is_lowercase( ) ) ) {
            user_agent.add_anomaly( Anomaly::Casing( directive.to_string( ), argument.to_string( ) ) );
            //NOTE: We don't ignore the casing anomaly, our goal is to be as permissive as possible
            directive.get_mut(0..1).map( | c |{ c.make_ascii_uppercase( ); &*c } );
        }

        match parse_directive( &directive, &argument ) {
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
                s.push_str( "\n" );
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

    fn directive_line( self, mut directive: String , argument: String ) -> Self {

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

        //HACK: Space saving is possible by moving the Casing anomaly check
        if directive.starts_with( | c: char |( c.is_lowercase( ) ) ) {
            robots.add_anomaly( Anomaly::Casing( directive.to_string( ), argument.to_string( ) ) );
            //NOTE: We don't ignore the casing anomaly, our goal is to be as permissive as possible
            directive.get_mut(0..1).map( | c |{ c.make_ascii_uppercase( ); &*c } );
        }

        match parse_directive( &directive, &argument ) {
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

        let mut ret = match self {
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
        };
        ret.agents.sort( );
        ret
    }
}
//--SNIP--

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

    //HACK: Best entry point into the code, understanding this is the key to adding any feature
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
                state = state.directive_line( l.to_string( ),
                                              r.trim_left_matches( | c:char |{ c.is_whitespace( ) ||
                                                                        c == ':' } ).to_string( ) );
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

    pub fn get_sitemaps( &self ) -> Vec<BaseUrl> {
        self.sitemaps.clone( )
    }

    pub fn get_allowances( &self, user_agent: &str ) -> Vec<Rule> {
        let agents = self.agents.iter( ).filter( | agent: &&UserAgent | //Y?
                              { agent.names.contains( &String::from( "*" ) ) ||
                                agent.names.contains( &user_agent.to_string( ) ) }
        );

        let mut ret = Vec::new( );

        for agent in agents {
            ret.append( &mut agent.rules.clone( ) );
        }

        ret
    }

    pub fn is_allowed( &self, url: BaseUrl, user_agent: &str ) -> bool {
        /* NOTE: The bias is to assume we are permitted until we see a Disallow directive at which
         * point this flips to false and only flips to true again if a more specific Allow directive is
         * found. This back and forth continues until we run out of applicable rules.
         */
        /* TODO: Timesaving is possible here by measuring the number of path segments the url contains
         * Since the specificity of a rule is equal to the number of path segments it contains we can
         * stop iteration short after we see a specificity greater than the path segments of the url
         */
        let mut bias = true;

        for rule in self.get_allowances( user_agent ) {
            if rule.applies( &url ) {
                bias = rule.is_allow( );
            }
        }
        bias
    }

}


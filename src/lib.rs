//
use reqwest::{ Response, StatusCode };

use baseurl::BaseUrl;

/// A set of observed anomalies in the robots.txt file
/// Anything not directly interacted with through this api is considered anomalous, and includes comments
/// and illegal (cross host) rule lines as well as unknown or unimplemented directives.
pub enum Anomaly {
    /// Any comment stored alongside some context, either the rest of the line the comment was found on
    /// or the line following
    Comment( String /*The comment*/, String /*The context*/ ),
    /// Any rule which refers to a remote host to the robots.txt file
    CrossHostRule( String, BaseUrl ),
    /// Rules whose names are not in the normal casing format, ie. "allow" rather than "Allow"
    Casing( String, BaseUrl ), // mostly here to guage if this type of error is common
    /// A known directive associated with a User-agent not normally associated as such, ie. a "Sitemap"
    MissSectioned( String, BaseUrl )
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
    Comment( RobotParser, String ), //We have a comment, but we can't see any context yet
    Agent( RobotParser, R_State ), //We are inside of a useragent section
    Normal( RobotParser ), //Any lines at the root level (those without a useragent association)
}

impl R_State {

    fn comment( &mut self, line: &str ) {

        self = match self {
            R_State::Comment( u, s ) => s.push_str( line ); R_State::Comment( u, s ),
            R_State::Normal( u ) => R_State::Comment( u, String::from( line ) ),
        };
    }

    fn context_comment( &mut self, context: &str, comment: &str ) {

        self = match self {
            R_State::Comment( u, s ) => {
                u.add_comment( s, context );
                u.add_comment( comment, context );
                State::Normal( u )
            }
            R_State::Normal( u ) => {
                u.add_comment( comment, context );
                State::Normal( u )
            }
        };
    }

    fn empty_line( &mut self ) -> UserAgent {

        match self {
            R_State::Comment( r, s ) => {
                r.add_comment( s, "" );
                r
            },
            R_State::Normal( u ) => {
                u
            },
        }
    }
}

impl State {

    fn comment( &mut self, line: &str ) {

        self = match self {
            State::Comment( r, s ) => s.push_str( line ); State::Comment( r, s ),
            State::Agent( r, s ) => s.comment( line ); State::Agent( r, s ),
            State::Normal( r ) => State::Comment( r, String::from( line ) ),
        };
    }

    fn context_comment( &mut self, context: &str, comment: &str ) {

        self = match self {
            State::Comment( r, s ) =>{
                r.add_comment( context, s );
                r.add_comment( context, comment );
                State::Normal( r )
            }
            State::Agent( r, s ) => {
                s.context_comment( context, comment );
                State::Agent( r, s )
            }
            State::Normal( r ) => {
                r.add_comment( context, comment );
                State::Normal( r )
            }
        };
    }

    fn empty_line( &mut self ) {

        self = match self {
            State::Comment( r, s ) => {
                State::Comment( r, s )
            }
            State::Agent( r, s ) => {
                r.add_agent( s.empty_line( ) );
                State::Normal( r )
            }
        }
    }

}

impl RobotsParser {

    pub fn guess_robots_url( &self ) -> BaseUrl {
        let ret = self.host.clone( );
        ret.set_path( "/robots.txt" );
        return ret;
    }

    pub fn from_response( response: Response ) -> RobotsParser {
        assert!( response.status( ).is_success( ) );

        let host = match BaseUrl::from( response.url( ) ) {
            Ok( u ) => u,
            Err( _e ) => panic!( ),
        };
        host.set_path( "/" );

        let text = match response.text( ) {
            Ok( t ) => t,
            Err( _e ) => panic!( ),
        }

        parse( host, text )
    }

    pub fn parse< S: Into< &str > >( host: BaseUrl, text: S ) -> RobotsParser {
        let lines = text.into( ).lines( );
        let mut ret = RobotsParser{
            host: host,
            sitemaps: Vec::new( ),
            agents: Vec::new( ),
            anomalies: Vec::new( ),
        };

        let mut state = State::Normal( ret );

        for _line in lines {
            //NOTE: in both of the split_at directives the split character goes into r
            let mut ( l, r );
            let mut line = _line.trim( );

            /***********
             * Empty Lines
             ******/
            if line.is_empty( ) {
                state.empty_line( );
                continue;
            }

            /***********
             * Comments
             ******/
            if line.starts_with( "#" ) {
                state.comment( line );
                continue;
            } else if line.contains( "#" ) {
                ( l, r ) = line.split_at( line.find( "#" ).unwrap( ) ).unwrap( );
                state.context_comment( l, r );
                line = l.trim( );
            }

            /***********
             * Directives
             ******/
            if line.contains( ":" ) {
                ( l, r ) = line.split_at( line.find( ":" ).unwrap( ) ).unwrap( );
                state.directive_line( l, r.trim_left_matches( ':' ) );
            } else {
                /***********
                 * Everything else
                 ******/
                state.anomaly( line );
            }

        }
    }
}


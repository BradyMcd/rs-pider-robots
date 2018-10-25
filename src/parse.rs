//
use base_url::BaseUrl;

use Anomaly;
use Rule;
use UserAgent;
use RobotsParser;

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

impl RobotsParser {

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
}

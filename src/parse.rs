//
// TODO: Line numbering, HACK commments

use std::cmp::Ordering;
use std::usize::MAX;

use base_url::BaseUrl;
use base_url::TryFrom;

use Anomaly;
use Rule;
use UserAgent;
use RobotsParser;

impl Rule{

    fn new( allowance: bool, mut path: String ) -> Rule {
        if path.is_empty( ) {
            path.push_str( "/" );
        }
        match allowance {
            true => {
                Rule::Allow( path )
            }
            false => {
                Rule::Disallow( path )
            }
        }
    }

    /***********
     * Ordering Helpers
     ******/
    pub fn is_allow( &self ) -> bool {
        //NOTE: if the path is empty that classically means that no paths are allowed/disallowed so we
        // flip the allowance returned, if no paths are disallowed then everything is allowed
        let mul = match self {
            Rule::Allow( path ) | Rule::Disallow( path ) => {
                if path.is_empty( ) {
                    false
                } else {
                    true
                }
            }
        };
        match self {
            Rule::Allow( _ ) => { mul == true }
            _ => { mul == false }
        }
    }

    pub fn path_specificity( path: &str ) -> usize {
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

/// Rules are ordered first by specificity then by their allowance. That means that the most specific
/// disallow rule is found first and the least specific allow rule is considered last.
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
    fn specificity( &self ) -> usize {
        self.names.iter().fold( MAX, | min: usize, name |{
            if min < name.len() { min } else { name.len( ) }
        } )
    }
}

impl PartialEq for UserAgent {
    fn eq( &self, rhs:&Self ) -> bool {
        self.names == rhs.names
    }
}
impl Eq for UserAgent {}

/// UserAgents are ordered by specificity. That means that names containing wildcards are considered
/// last when determining permissions while UserAgents with the longest full name is considered first.
impl PartialOrd for UserAgent {
    // TODO: possibly give this another look. Maybe I should care about name length more than number of
    // names
    fn partial_cmp( &self, rhs: &Self ) -> Option< Ordering > {
        let wildcard = String::from( "*" );

        if self.names == rhs.names{
            Some( Ordering::Equal )
        }else if self.names.contains( &wildcard ) {
            Some( Ordering::Greater )
        } else if rhs.names.contains( &wildcard ) {
            Some( Ordering::Less )
        }else {
            Some( self.specificity( ).cmp( &rhs.specificity( ) ).reverse( ) )
        }
    }
}

impl Ord for UserAgent {
    fn cmp( &self, rhs: &Self ) -> Ordering {
        self.partial_cmp( rhs ).unwrap( )
    }
}

#[allow(non_camel_case_types)]
enum R_State { //Recursed state; useragent sections don't recurse, they add
    Comment( UserAgent, String ),
    Normal( UserAgent ),
}

enum State {
    Comment( RobotsParser, String ), //We have a comment, but we can't see any context yet
    Agent( RobotsParser, R_State ), //We are inside of a useragent section
    Normal( RobotsParser ), //Any lines at the root level (those without a useragent association)
}

#[allow(non_camel_case_types)]
enum DirectiveResult {
    Ok_UserAgent( String ),
    Ok_Rule( Rule ),
    Ok_Sitemap( BaseUrl ),
    //Ok_RequestRate( u32 ),
    //Ok_CrawlDelay( u32 ),
    Err_BadArg(),
    Unknown(),
}

fn parse_directive( directive: &str, argument: &str ) -> DirectiveResult {
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
            let url = BaseUrl::try_from( argument );
            if url.is_ok( ) {
                DirectiveResult::Ok_Sitemap( url.unwrap( ) )
            } else {
                DirectiveResult::Err_BadArg()
            }
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

        //NOTE: This isn't going to capture all casing anomalies on the web, "User-Agent" for example
        // can be found on many very popular sites, even in the same file where "User-agent" is used
        //NOTE: Space saving is possible by moving the Casing anomaly check
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
            DirectiveResult::Err_BadArg() => {
                user_agent.add_anomaly( Anomaly::BadArgument( directive.to_string( ),
                                                              argument.to_string( ) ) );
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
                robots.add_sitemap( s );
                State::Normal( robots )
            }
            DirectiveResult::Err_BadArg( ) => {
                robots.add_anomaly( Anomaly::BadArgument( directive.to_string( ),
                                                          argument.to_string( ) ) );
                State::Normal( robots )
            }
//            DirectiveResult::Ok_RequestRate( r ) => {}
//            DirectiveResult::Ok_CrawlDelay( d ) = > {}
            DirectiveResult::Unknown( ) => {
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
    //HACK: This is the function to understand if you want to add a feature
    pub fn parse< S: Into<String> >( host: BaseUrl, text: S ) -> Self {
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


/***********
 * Unit Tests
 ******/
#[cfg(test)]
mod tests {
    use super::*;

    /***********
     * Rule
     ******/
    #[test]
    fn rule_creation( ) {
        let rule = Rule::new( true, String::from( "/foo/" ) );

        assert_eq!( Rule::Allow( String::from( "/foo/" ) ), rule );
    }

    #[test]
    fn rule_specificity( ) {
        let mut path = String::from( "" );
        let rule_a  = Rule::new( true, path.clone( ) );

        assert_eq!( rule_a.specificity( ), 0 );

        path.push_str( "/foo" );
        let rule_b = Rule::new( true, path.clone( ) );
        path.push_str( "/" );
        let rule_c = Rule::new( false, path.clone( ) );

        assert_eq!( rule_b.specificity( ), rule_c.specificity( ) );
    }

    #[test]
    fn rule_ordering( ) {
        let mut path = String::from( "/" );
        let rule_a = Rule::new( false, path.clone( ) );
        let rule_b = Rule::new( true, path.clone( ) );

        path.push_str( "foo" );

        let rule_c = Rule::new( false, path.clone( ) );

        path.push_str( "/bar" );

        let rule_d = Rule::new( true, path.clone( ) );

        let mut rule_vec_a = vec![ rule_b.clone( ), rule_a.clone( ),
                                   rule_d.clone( ), rule_c.clone( ) ];

        let mut rule_vec_b = vec![ rule_a.clone( ), rule_c.clone( ),
                                   rule_b.clone( ), rule_d.clone( )];

        rule_vec_a.sort( );
        rule_vec_b.sort( );

        assert_eq!( rule_vec_a, rule_vec_b );
    }

    /***********
     * UserAgent
     ******/
    #[test]
    fn useragent_ordering( ) {
        let ua_1 = UserAgent::new( String::from( "*" ) );
        let ua_2 = UserAgent::new( String::from( "foogle" ) );
        let ua_3 = UserAgent::new( String::from( "foogle-news" ) );

        assert!( ua_1 > ua_2 );
        assert!( ua_2 > ua_3 );

    }
}

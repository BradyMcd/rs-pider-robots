//
// TODO: Getters, Tests, Documentation, HACK comments

extern crate base_url;
extern crate multi_order;

#[cfg( feature="fetch" )]
extern crate reqwest;

use std::convert::*;

use std::fmt::{ Formatter, Display };
use std::fmt::Result as DisplayResult;

use base_url::BaseUrl;
use multi_order::Enum;
use multi_order::interior::sectioned_queue::*;

#[cfg( fetch ) ]
use reqwest::{ Response };

mod path_match;
use path_match::*;
mod parse;
/* Still here so I can figure out how to move documentation around
#[derive( PartialEq, Debug, Clone )]
pub enum Anomaly {
    /// Any comment stored alongside some context, either the rest of the line the comment was found on
    /// or the line following. Context strings may be observed twice if a block comment is placed above
    /// a line with a directive and comment.
    Comment( String /*The comment*/, String /*The context*/ ),
    /// Rules whose names are not in the normal casing format, ie. "foo-bar" or "fOo-BaR" rather than
    /// "Foo-bar"
    Casing( String, String ),
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
    /// Any argument which is in a bad format ie. a non-url sitemap or non-integer delay
    BadArgument( String, String ),
    /// Any line which isn't in the standard format for a robots.txt file, ie. a line without a ':'
    /// separator which is not a comment
    UnknownFormat( String ),
}
 */

macro_rules! MatchHelper {
    ( $type: ty ) => {
        _
    }
}
macro_rules! FormatHelper {
    ( $id: ident ) => {
        "{}"
    }
}


macro_rules! anomaly_enum {
    ( $( $id:ident ; ( $( $arg:ty ),+ ) ( $( $bind: ident ),+ ) ;
         $header:expr ; $fmt:expr ),+ ) => (
        /// A set of observed anomalies in the robots.txt file
        /// Anything not directly interacted with through the rest of this api is considered anomalous, and
        /// includes comments and ambiguously placed rule lines as well as unknown or unimplemented
        /// directives.
        #[derive( PartialEq, Debug, Clone )]
        pub enum Anomaly{
            $(
                $id ( $( $arg ),+ )
            ),+
        }

        impl Anomaly {
            fn header_string( &self ) -> &str {
                match self {
                    $(
                        Anomaly::$id ( $( MatchHelper!( $arg ) ),+ ) => $header,
                    )+
                }
            }
            fn body_string( &self ) -> String {
                match self {
                    $(
                        Anomaly::$id ( $( $bind ),+ ) => format!(
                            concat!( $( FormatHelper!( $bind ) ),+ ), $( $bind ),+ ),
                        )+
                    }
           }
            fn describe( &self ) -> String {
                match self {
                    $(
                        Anomaly::$id ( $( $bind ),+ ) => format!( $fmt, $( $bind ),+ ),
                    )+
                }
            }
        }
    )
}


anomaly_enum! (
    Comment ; ( String, String ) ( comment, context ) ;
    "Comments:" ; "{1} \nWas commented on:\n{0}",
    Casing ; ( String, String ) ( directive, _argument ) ;
    "Non-standard casing in directives:" ; "Directive {0}:{1} has odd casing",
    OrphanRule ; ( Rule ) ( rule ) ;
    "Rules found outside of User-agent sections:" ; "Orphaned rule line: {}",
    RecursedUserAgent ; ( String ) ( agent );
    "User-agents found after a rule line:" ; "User-agent {} was found nested ambiguously and ignored",
    RedundantWildcardUserAgent ; ( String ) ( agent ) ;
    "Specified User-agents in a wildcard section:" ; "User-agent {} was mentioned after a wildcard",
    MissSectionedDirective ; ( String, String ) ( directive, argument ) ;
    "Root directives found in a User-agent section:" ; "Directive {0}: {1} found under a User-agent",
    UnknownDirective ; ( String, String ) ( directive, argument );
    "Unimplemented or unknown directives found:" ; "Unknown directive: {0}: {1}",
    BadArgument ; ( String, String ) ( directive, argument ) ;
    "Poorly formatted arguments:" ; "The argument {1} couldn't be parsed for a {0} directive",
    UnknownFormat ; ( String ) ( line ) ;
    "Poorly formatted lines:" ; "Unknown line format: {}"
);

impl Display for Anomaly {
    fn fmt( &self, f: &mut Formatter ) -> DisplayResult {
        write!( f, "{}", self.describe( ) )
    }
}

/// Represents a Rule line found in a User-agent section
#[derive( Debug, Clone, PartialEq, Eq )]
pub enum Rule {
    Allow( String ),
    Disallow( String ),
    /* TODO:
     * Crawl-delay
     * Request-rate
     */

}

impl Rule {

    fn applies( &self, url: &BaseUrl ) -> bool {
        let url_specificity = Self::path_specificity( url.path( ) );
        let self_specificity;
        let url_path = url.path( ).split( '/' );
        let self_path = match self {
            Rule::Allow( path ) | Rule::Disallow( path ) => {
                if path == "/" || path.is_empty( ) { return true; }
                self_specificity = Self::path_specificity( path );
                path.split( '/' )
            }
        };

        if url_specificity < self_specificity {
            false
        } else {
            for segments in url_path.zip( self_path ) {
                let ( url_seg, self_seg ) = segments;
                if !match_with_asterisk( url_seg, self_seg ) {
                    return false;
                }
            }
            true
        }
    }
}

impl Display for Rule {
    fn fmt( &self, formatter: &mut Formatter ) -> DisplayResult {
        match self {
            Rule::Allow( path ) => { write!( formatter, "Allow: {}", path ) }
            Rule::Disallow( path ) => { write!( formatter, "Disallow: {}", path ) }
        }
    }
}

/// A User-agent section and all names, rules and anomalies associated
#[derive( Debug, Clone )]
struct UserAgent {
    names: Vec< String >,
    rules: Vec< Rule >,
    anomalies: Vec< Anomaly >,
}

impl UserAgent {

    fn new( mut agent: String ) -> Self {
        if agent.is_empty( ) {
            agent.push_str( "*" );
        }
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

    fn applies( &self, user_agent: &str ) -> bool {

        self.names.iter( ).any( | name |{
            user_agent.starts_with( name ) || name == "*"
        } )
    }
}
/// Represents a parsed robots.txt file
pub struct RobotsParser {
    host: BaseUrl,
    sitemaps: Vec<BaseUrl>,
    agents: Vec<UserAgent>,
    anomalies: Vec< Anomaly >,
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

    fn get_allowances( &self, user_agent: &str ) -> Vec<Rule> {
        let agents = self.agents.iter( ).filter(
            | agent: &&UserAgent | { agent.applies( user_agent ) }
        );

       let mut ret = Vec::new( );

        for agent in agents {
            ret.append( &mut agent.rules.clone( ) );
        }

        ret
    }

    /***********
     * Creation
     ******/

    // NOTE: This function being a function makes sense, where it is makes less sense
    pub fn guess_robots_url( &self ) -> BaseUrl {
        let mut ret = self.host.clone( );
        ret.strip( );
        ret.set_path( "/robots.txt" );
        return ret;
    }

    #[cfg( feature="fetch" )]
    pub fn from_response( mut response: Response ) -> Self {
        assert!( response.status( ).is_success( ) );

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

    pub fn from_stringable < S: Into< String > > ( stringable: S, host: BaseUrl ) -> Self {

        let text = stringable.into( );

        Self::parse( host, text )
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

    /// Retreives any anomalies appearing at the top level of the robots.txt document. Any anomaly not
    /// observed inside of a User-agent section is returned by this function and may contain things
    /// like orphaned and unimplemented directives.
    pub fn get_toplevel_anomalies( &self ) -> &Vec< Anomaly > {
        &self.anomalies
    }

    /// Retreives any anomalies appearing under a User-agent section as determined by the agent's
    /// .applies( ) function. That means that agent names starting with the supplied string are
    /// returned and also that the asterisk(*) character can be supplied to indicate all agents.
    pub fn get_agent_anomalies( &self, user_agent: &str ) -> Vec< &Anomaly > {
        let agents = self.agents.iter( ).filter(
            | agent: &&UserAgent | { agent.applies( user_agent ) }
        );

        let mut ret = Vec::new( );

        for agent in agents {
            ret.extend( agent.anomalies.iter( ) )
        }

        ret
    }

    /// Retrieves a set of all the anomalous lines which were found when parsing the robots.txt file
    /// Note that Anomalies are not necessarily reported in the order they were found in the document
    /// and are instead sorted
    pub fn get_all_anomalies( &self ) -> Vec<&Anomaly> {

        let mut ret = Vec::new( );

        ret.extend( self.anomalies.iter( ) );
        ret.extend( self.get_agent_anomalies( "*" ) );

        ret
    }

    /// Given a url and a user agent string determines if this robots.txt disallows browsing to that
    /// url. This is generally understood as more of a suggestion than a rule.
    //HACK: Can we combine the search through the UserAgents and the search for allowances in a way
    // which is clean?
    pub fn is_allowed( &self, url: &BaseUrl, user_agent: &str ) -> bool {

        assert!( url.host( ) == self.host_url( ).host( ) );

        for rule in self.get_allowances( user_agent ) {
            if rule.applies( &url ) {
                return rule.is_allow( );
            }
        }
        true
    }
}

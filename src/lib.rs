//
// TODO: Getters, Tests, Documentation, HACK comments

extern crate base_url;

#[cfg( fetch )]
extern crate reqwest;

use std::convert::*;

use std::fmt::{ Formatter, Display };
use std::fmt::Result as DisplayResult;

#[cfg( fetch ) ]
use reqwest::{ Response };

use base_url::BaseUrl;

mod path_match;
use path_match::*;
mod parse;


//I wanna re-arrange this
/// A set of observed anomalies in the robots.txt file
/// Anything not directly interacted with through the rest of this api is considered anomalous, and
/// includes comments and illegal (cross host) rule lines as well as unknown or unimplemented
/// directives.
#[derive( Debug, Clone )]
pub enum Anomaly {
    /// Any comment stored alongside some context, either the rest of the line the comment was found on
    /// or the line following. Context strings may be observed twice if a block comment is placed above
    /// a line with a directive and comment.
    Comment( String /*The comment*/, String /*The context*/ ),
    /// Rules whose names are not in the normal casing format, ie. "foo" rather than "Foo"
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

//I really wanna rearrange this
impl Display for Anomaly {
    fn fmt( &self, formatter: &mut Formatter ) -> DisplayResult {
        match self {
            Anomaly::Comment( cmnt, ctxt ) => {
                if ctxt.contains( "\n" ) || ctxt == "[EOF]" {
                    write!( formatter, "{}{}", cmnt, ctxt )
                } else {
                    write!( formatter, "{}{}", ctxt, cmnt )
                }
            }
            Anomaly::Casing( rule, path ) => {
                write!( formatter, "Nonstandard casing: {}: {}", rule, path )
            }
            Anomaly::OrphanRule( rule ) => {
                write!( formatter, "Orphaned Rule: {}", rule )
            }
            Anomaly::RecursedUserAgent( name ) => {
                write!( formatter, "Recursed User-agent: {}", name )
            }
            Anomaly::RedundantWildcardUserAgent( name ) => {
                write!( formatter, "Specific User-agent: {} found after a wildcard", name )
            }
            Anomaly::MissSectionedDirective( drctv, arg ) => {
                write!( formatter, "{}: {}", drctv, arg )
            }
            Anomaly::UnknownDirective( drctv, arg ) => {
                write!( formatter, "Unimplemented directive: {}: {}", drctv, arg )
            }
            Anomaly::BadArgument( drctv, arg ) => {
                write!( formatter, "Bad argument: {}: {}", drctv, arg )
            }
            Anomaly::UnknownFormat( line ) => {
                write!( formatter, "Unknown line: {}", line )
            }
        }
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

    // NOTE: This function being a function makes sense, the implementation makes no sense
    pub fn guess_robots_url( &self ) -> BaseUrl {
        let mut ret = self.host.clone( );
        ret.strip( );
        ret.set_path( "/robots.txt" );
        return ret;
    }

    #[cfg( fetch )]
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

    //NOTE: in the anomaly getters I should be less afraid of returning an iterator
    pub fn get_toplevel_anomalies( &self ) -> &Vec< Anomaly > {
        &self.anomalies
    }

    pub fn get_agent_anomalies( &self, user_agent: &str ) -> std::slice::Iter< &Anomaly > {
        let agents = self.agents.iter( ).filter(
            | agent: &&UserAgent | { agent.applies( user_agent ) }
        );

        let mut ret = Vec::new( ).iter( );
        for agent in agents{
            ret.chain( agent.anomalies.as_ref() );
        }
        ret
    }

    //NOTE:This is the getter to work on.
    pub fn get_all_anomalies( &self ) -> Vec<Anomaly> {

        let mut ret = self.anomalies.clone( );

        for agent in self.agents.iter( ) {
            ret.append( &mut agent.anomalies.clone( ) );
        }

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

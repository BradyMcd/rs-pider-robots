//
// TODO: Getters, Tests, Documentation, HACK comments

extern crate reqwest;
extern crate base_url;
extern crate try_from;

use std::convert::*;

use reqwest::{ Response };

use try_from::TryFrom;
use base_url::BaseUrl;

mod parse;

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

    pub fn guess_robots_url( &self ) -> BaseUrl {
        let mut ret = self.host.clone( );
        ret.set_path( "/robots.txt" );
        return ret;
    }

    pub fn from_response( mut response: Response ) -> Self {
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

    /***********
     * Getters
     ******/

    pub fn host_url( &self ) -> BaseUrl {
        self.host.clone( )
    }

    pub fn get_sitemaps( &self ) -> Vec<BaseUrl> {
        self.sitemaps.clone( )
    }

    /// Given a url and a user agent string determines if this robots.txt disallows browsing to that
    /// url. This is generally understood as more of a suggestion than a rule.
    //HACK: Can we combine the search through the UserAgents and the search for allowances in a way
    // which is clean?
    pub fn is_allowed( &self, url: BaseUrl, user_agent: &str ) -> bool {

        for rule in self.get_allowances( user_agent ) {
            if rule.applies( &url ) {
                return rule.is_allow( );
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    
}

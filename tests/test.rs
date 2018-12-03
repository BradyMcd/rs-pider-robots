
extern crate rs_pider_robots;
extern crate base_url;
extern crate try_from;

use rs_pider_robots::*;

use base_url::BaseUrl;
use try_from::TryFrom;

mod data;
use data::{ ROBOTS_SIMPLE, ROBOTS_OVERLAPPING, ROBOTS_SITEMAPS, ROBOTS_WILD };

#[test]
fn test_simple_robots( ) {

    let host = BaseUrl::try_from( "https://example.com/" ).ok( ).unwrap( );

    let simple = RobotsParser::from_stringable( ROBOTS_SIMPLE, host );

    assert!(
        !simple.is_allowed( &BaseUrl::try_from( "https://example.com/a/path/" ).ok( ).unwrap( ), "bot")
    );
}

#[test]
fn test_overlapping_robots( ) {

    let host = BaseUrl::try_from( "https://example.com/" ).ok( ).unwrap( );

    let overlapping = RobotsParser::from_stringable( ROBOTS_OVERLAPPING, host );

    let url1 = BaseUrl::try_from( "https://example.com/foo" ).ok( ).unwrap( );
    let url2 = BaseUrl::try_from( "https://example.com/foo/bar" ).ok( ).unwrap( );
    let url3 = BaseUrl::try_from( "https://example.com/foo/bar/baz" ).ok( ).unwrap( );

    assert!( !overlapping.is_allowed( &url1, "Bot" ) );
    assert!( overlapping.is_allowed( &url2, "Bot" ) );
    assert!( overlapping.is_allowed( &url2, "Bot-1" ) );
    assert!( !overlapping.is_allowed( &url2, "aBot" ) );
    assert!( !overlapping.is_allowed( &url3, "Bot" ) );
    assert!( overlapping.is_allowed( &url3, "Bot-1" ) );
}

#[test]
fn test_sitemaps_robots( ) {

    let host = BaseUrl::try_from( "https://example.web" ).ok( ).unwrap( );

    let sitemaps = RobotsParser::from_stringable( ROBOTS_SITEMAPS, host );

    assert!( sitemaps.get_sitemaps( ).len( ) == 3 );
}

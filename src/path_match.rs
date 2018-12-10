//
pub fn match_with_asterisk( haystack: &str, needle: &str ) -> bool {

    if !needle.contains( '*' ) {
        return haystack == needle;
    }

    if needle == "*" {
        return true;
    }

    let mut loc = 0;
    let segments = if needle.starts_with( '*' ) {
        let mut _segments = needle.split( '*' );
        let first_seg = _segments.next( ).unwrap( );
        if !haystack.starts_with( first_seg ) { return false; }
        loc += first_seg.len( );
        _segments
    } else {
        needle.split( '*' )
    };

    for seg in segments {
        if seg == "" { /*DONOTHING*/ }
        else {
            match prefix_asterisk( seg, &haystack[ loc.. ], 0 ) {
                Some( i ) => {
                    loc += i
                }
                None => {
                    return false;
                }
            }
        }
    }
    true
}

fn prefix_asterisk( suffix: &str, haystack:&str, loc: usize ) -> Option< usize > {

    if haystack.len( ) < suffix.len( ) {
        return None;
    }

    if haystack.starts_with( suffix ) {
        return Some( loc );
    }

    return prefix_asterisk( suffix, &haystack[1..], loc + 1 );
}


#[cfg( test )]
mod tests{

    use super::*;

    #[test]
    fn asterisk_only( ) {
        assert!( match_with_asterisk( "This can be literally anything", "*" ) );
        assert!( match_with_asterisk( "No really, anything +-=*//\\", "*" ) );
        assert!( match_with_asterisk( "", "*" ) );
    }

    #[test]
    fn no_asterisk( ) {
        assert!( match_with_asterisk( "No Asterisk", "No Asterisk" ) );
    }

    #[test]
    fn leading_asterisk( ) {
        assert!( match_with_asterisk( "Target", "*Target" ) );
        assert!( match_with_asterisk( "Some things we don't care about and the Target", "*Target" ) );
        assert!( match_with_asterisk( "The Target needs to be last to match", "*Target" ) );
        assert!( !match_with_asterisk( "Really just things we don't care about", "*Target" ) );
    }

    #[test]
    fn trailing_asterisk( ) {
        assert!( match_with_asterisk( "Target", "Target*" ) );
        assert!( match_with_asterisk( "Target and some things we don't care about", "Target*" ) );
        assert!( !match_with_asterisk( "We care about Target, but this won't match", "Target*" ) );
        assert!( !match_with_asterisk( "No instance of the string we want", "Target*" ) );
    }

    #[test]
    fn segmented_asterisks( ) {
        assert!( match_with_asterisk( "A bit more complex, but still works",
                                      "A bit*but still*work*" ) );
        assert!( !match_with_asterisk( "more complex by a bit and doesn't work",
                                       "*a bit*more complex*work*" ) );
    }

    #[test]
    fn redundant_asterisks( ) {
        assert!( match_with_asterisk( "This should match", "**sh**ma*" ) );
        assert!( match_with_asterisk( "Doesn't match", "**at**oe*" ) );
    }

}

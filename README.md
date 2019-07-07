# rs_pider_robots

This is a simple robots.txt parser module for the Rust language. It's designed to be permissive and, in
one way or the other, record everything it sees in a given document. Comments, unknown directives, 
unimplemented, misplaced directives, all are recorded as an Anomaly and stored in a Vec based on where
they are noted. The goal of this project is to later act as a module in a larger web spider, as that 
project evolves so will this. Because it's meant for use reading the web no facilities are provided to
create a robots.txt file of your own and no serialization of the original file is stored only the 
encoding.

## Permissions, and how they are determined

Permissions are determined in order of specificity, whichever rule is considered first will be taken as
the intended meaning of the robots.txt document. User-agent sections which name an agent will be 
considered before User-agent sections which use a wildcard. User-agent names are judged as being the 
string a given agent starts with, that means a User-agent section naming "a" will match your supplied 
user agent strings of not only "a" but also "aa", "azzzz", "a3.141596...". As for directive arguments, 
the most specific argument is the one which contains the most path segments.

Both the unit tests found in parse.rs and the tests in the test directory contain concrete examples of
this behavior.

## Unimplemented behaviors

Currently 3 common-ish directives are not implemented, Host, Crawl-delay, and Request-rate. Crawl-delay
 and Request-rate are both planned for a later version and as I use the framework more I might find 
enough Host directives to warrant their inclusion as well. In the meantime though you can technically
implement these yourself by ```.filter()```ing for the UnknownDirective Anomaly and matching on it's contents.

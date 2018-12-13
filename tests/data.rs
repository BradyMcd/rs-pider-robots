// A set of robots.txt files as rust &'static str constants

//The simplest robots.txt file
pub static ROBOTS_SIMPLE: &'static str =
    "User-agent:* \n\
     Disallow:/ \n";

//A robots.txt file with User-agent names which may overlap and a complex Allow/Disallow interaction
pub static ROBOTS_OVERLAPPING: &'static str =
    "User-agent:* \n\
     Disallow:/foo \n\
     \n\
     User-agent:Bot \n\
     Allow:/foo/bar \n\
     \n\
     User-agent:Bot-1 \n\
     Disallow:/foo/bar/baz \n";
//To summarize our expectations here, nobody should have permission to /foo, bots whose name start with
// Bot are allowed in /foo/bar but if they're named Bot-1 they shouldn't access the /baz subdirectory
// there


//A robots.txt file with Sitemaps specified
pub static ROBOTS_SITEMAPS: &'static str =
    "Sitemap:http://www.example.web/sitemap.xml \n\
     Sitemap:http://www.example.web/sitemaps/archive1.xml \n\
     Sitemap:https://www.example.web/a/man/with/three/buttocks/sitemap.xml \n";

//A slightly more typical robots.txt file, with more specific permissions given than just disallow all
// built out of a few samples thrown together to hopefully be representative of a file you would expect
// to find in the wild including references to pop-culture, convoluted sitemapping setups and wildcard
// Disallow directives
pub static ROBOTS_WILD: &'static str =
    "User-agent:* \n\
     Disallow:/admin/ \n\
     Disallow:/cgi/ \n\
     Disallow:/beta/ \n\
     Disallow:/*/comments/*/ \n\
     Disallow:/*.embed \n\
     \n\
     # 80legs \n\
     User-agent: 008 \n\
     User-agent: voltron \n\
     Disallow:/ \n\
     \n\
     User-agent: bender \n\
     Disallow: /my_shiny_metal_ass \n\
     \n\
     Sitemap: https://www.example.web/sitemaps/sitemap-section.xml \n\
     Sitemap: https://www.example.web/sitemaps/foo/index.xml \n";


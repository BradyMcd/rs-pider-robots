// A set of robots.txt files as rust &'static str constants

//The simplest robots.txt file
static ROBOTS_SIMPLE: &'static str = "User-agent:* \
                                      Disallow:/";

//A robots.txt file with User-agent names which may overlap and a complex Allow/Disallow interaction
static ROBOTS_OVERLAPPING: &'static str = "User-agent:* \
                                           Disallow:/foo \
                                           \
                                           User-agent:Bot \
                                           Allow:/foo/bar \
                                           \
                                           User-agent:Bot-1 \
                                           Disallow:/foo/bar/baz";
//To summarize our expectations here, nobody should have permission to /foo, bots whose name start with
// Bot are allowed in /foo/bar but if they're named Bot-1 they shouldn't access the /baz subdirectory
// there


//A robots.txt file with Sitemaps specified
static ROBOTS_SITEMAPS: &'static str = "Sitemap:www.example.web/sitemap.xml \
                                        Sitemap:www.example.web/sitemaps/archive1.xml \
                                        Sitemap:www.example.web/a/man/with/three/buttocks/sitemap.xml";

//A slightly more typical robots.txt file, with more specific permissions given than just disallow all
// built out of a few samples thrown together to hopefully be representative of a file you would expect
// to find in the wild including references to pop-culture, convoluted sitemapping setups and wildcard
// Disallow directives
static ROBOTS_WILD: &'static str = "User-agent:*  \
                                    Disallow:/admin/ \
                                    Disallow:/cgi/ \
                                    Disallow:/beta/\
                                    Disallow:/*/comments/*/ \
                                    Disallow:/*.embed
                                    \
                                    # 80legs \
                                    User-agent: 008 \
                                    User-agent: voltron \
                                    Disallow:/ \
                                    \
                                    User-agent: bender\
                                    Disallow: /my_shiny_metal_ass \
                                    \
                                    Sitemap: www.example.web/sitemaps/sitemap-section.xml \
                                    Sitemap: www.example.web/sitemaps/foo/index.xml";


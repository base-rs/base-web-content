use base_web_content::Content;

#[derive(Content)]
struct Profile {
    #[content(accept("text/plain"))]
    biography: String,
}

fn main() {}

use base_web_content::Content;

#[derive(Content)]
#[content(formats("application/json"))]
struct Profile {
    name: String,
}

fn main() {}

use base_web_content::Content;

#[derive(Content)]
struct Profile {
    tags: Vec<Option<String>>,
}

fn main() {}

use base_web_content::Content;

#[derive(Content)]
struct Profile {
    #[content(count_limit = 0)]
    tags: Vec<String>,
}

fn main() {}

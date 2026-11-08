use base_web_content::Content;

#[derive(Content)]
struct Profile {
    #[content(count_limit = 4)]
    name: String,
}

fn main() {}

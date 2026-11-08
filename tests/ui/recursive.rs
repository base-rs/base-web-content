use base_web_content::Content;

#[derive(Content)]
struct Node {
    name: String,
    children: Vec<Node>,
}

fn main() {}

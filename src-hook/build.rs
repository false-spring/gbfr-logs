fn main() {
    let res = winres::WindowsResource::new();
    res.compile().unwrap();
}

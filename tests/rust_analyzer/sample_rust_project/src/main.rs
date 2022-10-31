mod other_file;

struct A {}

impl A {
    fn impl_method(&self) {
        other_file::other_file_method();
    }
}

fn main() {
    foo();
    (A {}).impl_method();
}

fn foo() {
    fn in_foo() {
        (A {}).impl_method();
    }

    in_foo();
}

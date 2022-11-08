package sample;

public class App {
    public static void main(String[] args) {
        foo();
        new App().method();
    }

    @Override
    public String toString() {
        return super.toString();
    }

    public void method() {
        new OtherFile().otherFileMethod();
    }

    public static void foo() {
        new App().method();
    }
}

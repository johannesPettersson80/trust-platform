Output docs/public/assets/images/terminal/install-version-checks.gif

Set Shell "bash"
Set FontSize 20
Set Width 1100
Set Height 620
Set Padding 24
Set Margin 12
Set MarginFill "#f5efe4"
Set BorderRadius 12
Set WindowBar Colorful
Set TypingSpeed 20ms

Type "export PATH=/workspaces/trust-platform/target/debug:$PATH"
Enter
Sleep 800ms

Type "command -v trust-lsp"
Enter
Sleep 800ms

Type "command -v trust-runtime"
Enter
Sleep 800ms

Type "command -v trust-debug"
Enter
Sleep 800ms

Type "command -v trust-harness"
Enter
Sleep 800ms

Type "trust-runtime --version"
Enter
Sleep 800ms

Type "trust-runtime --help | sed -n '1,10p'"
Enter
Sleep 1800ms

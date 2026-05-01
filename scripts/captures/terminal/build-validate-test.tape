Output docs/public/assets/images/terminal/build-validate-test.gif

Set Shell "bash"
Set FontSize 18
Set Width 1320
Set Height 780
Set Padding 24
Set Margin 12
Set MarginFill "#f5efe4"
Set BorderRadius 12
Set WindowBar Colorful
Set TypingSpeed 16ms

Type "./target/debug/trust-runtime build --project examples/memory_marker_counter --sources src"
Enter
Sleep 1200ms

Type "./target/debug/trust-runtime validate --project examples/memory_marker_counter"
Enter
Sleep 1200ms

Type "./target/debug/trust-dev test --project examples/memory_marker_counter --output human"
Enter
Sleep 2200ms

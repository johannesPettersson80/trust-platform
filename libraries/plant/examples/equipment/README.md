# Every Plant block

One program with one instance of each block, called once per scan. Feedback is switched off
where the example has no hardware, so every device can be started, stopped, opened and positioned
from its panel; connect the feedback inputs on a real machine and leave `HasFeedback` at TRUE.
A small travel model stands in for the motor valve's limit switches: it reaches an end five
seconds after it starts moving.

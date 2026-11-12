### Fixed

- An input of a method that implements a method an interface inherits through `EXTENDS`, such as a prototype of
  `itfMqttBuild` in a block that implements `itfMqttPayloadBuilder EXTENDS itfMqttBuild, itfMqttResult`, is no longer
  reported as an unused parameter (`W002`): the interface fixes the method's parameters, as it does for its own
  prototypes. The same holds for a method of a block that extends a block implementing the interface (#482).

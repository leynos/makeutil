Feature: Parse one GNU Makefile into JSON facts

  Scenario: Parse a complete Makefile by path
    Given a complete GNU Makefile fixture
    When makeutil parses the fixture by path
    Then stdout contains one schema version 1 JSON document
    And the process exits with code 0
    And stderr is empty

  Scenario: Parse complete source from standard input
    Given complete GNU Makefile source on standard input
    When makeutil parses dash with stdin filename Makefile
    Then the report source path is Makefile
    And the process exits with code 0

  Scenario: Reject a missing input path
    Given a path that does not exist
    When makeutil attempts to parse the missing path
    Then stdout is empty
    And stderr reports the source-open operation
    And the process exits with code 2

  Scenario: Reject an invalid invocation
    Given an invalid parse invocation
    When makeutil processes the invocation
    Then stdout is empty
    And stderr reports the cli operation
    And the process exits with code 2

  Scenario: Display help
    Given a help display request
    When makeutil processes the invocation
    Then stdout contains command help
    And stderr is empty
    And the process exits with code 0

  Scenario: Display version
    Given a version display request
    When makeutil processes the invocation
    Then stdout contains the makeutil version
    And stderr is empty
    And the process exits with code 0

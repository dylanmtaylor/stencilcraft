@GEO-05
Feature: Operations panel

  Scenario: hammering a geometry row creates an operation over its shapes
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I hammer the geometry row "outer"
    Then the document has 2 operations
    And the last operation has kind "cut"
    And the last operation has members 1, 2, 3
    And the operations panel shows "Pass 2"
    And the geometry panel shows "#3, #1 #2"

  @UI-04
  Scenario: reordering an operation changes its pass order
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I move operation 2 up
    Then operation 1 has members 1, 2, 3
    And the document has 2 operations

  @UI-05
  Scenario: editing an operation's settings
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I open the settings for operation 2
    Then "Power" is on screen
    When I set the power to 40
    And I click "Apply"
    Then operation 2 has power 40

  @UI-05
  Scenario: changing an operation kind
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I set operation 2 to kind "engrave"
    Then operation 2 has kind "engrave"

  @UI-05
  Scenario: locking an operation
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I toggle the lock on operation 2
    Then operation 2 is locked

  @UI-05
  Scenario: deleting an operation
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I delete operation 2
    Then the document has 1 operation

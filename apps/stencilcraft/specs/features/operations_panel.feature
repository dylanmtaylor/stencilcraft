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

  @gap @UI-04
  Scenario: reordering an operation changes its pass order
    Given a nested fixture with groups "outer" and "inner"
    When I click the "Move up" button for operation 2
    Then operation 2 has members 1, 2, 3

  @gap @UI-05
  Scenario: editing an operation's settings
    Given a nested fixture with groups "outer" and "inner"
    When I click the "Settings" button for operation 1
    Then "Power" is on screen

  @gap @UI-05
  Scenario: locking an operation
    Given a nested fixture with groups "outer" and "inner"
    When I click the "Lock" button for operation 1
    Then operation 1 is locked

  @gap @UI-05
  Scenario: deleting an operation
    Given a nested fixture with groups "outer" and "inner"
    When I click the "Delete" button for operation 1
    Then the document has 0 operations

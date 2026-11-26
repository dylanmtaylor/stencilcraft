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
    And I click "Close"
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

  @UI-04
  Scenario: expanding an operation lists its member shapes
    Given a nested fixture with groups "outer" and "inner"
    When I expand operation 1
    Then the operations panel shows "Member #1 of cut"
    And the operations panel shows "Member #2 of cut"
    And the operations panel shows "Member #3 of cut"

  @UI-04
  Scenario: removing a member from the expanded operation row
    Given a nested fixture with groups "outer" and "inner"
    When I expand operation 1
    And I remove member #1 from operation 1
    Then operation 1 has members 2, 3

  @UI-05
  Scenario: the settings popover changes kind, lock and view filter
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I open the settings for operation 2
    Then "Kind" is on screen
    And "Lock Cut 2" is on screen
    And "Show Cut 2 in geometry view" is on screen
    When I set operation 2 to kind "engrave"
    And I toggle the lock on operation 2
    Then operation 2 has kind "engrave"
    And operation 2 is locked

  @UI-05
  Scenario: the colour swatch sets the operation colour
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I open the colour of operation 2
    And I set the colour to "#ff0000"
    And I close the colour picker
    Then operation 2 has colour "#ff0000"

  @UI-05
  Scenario: the trash button deletes an operation
    Given a nested fixture with groups "outer" and "inner"
    When I hammer the geometry row "outer"
    And I click "Delete operation cut"
    Then the document has 1 operation

  @UI-04
  Scenario: the plus button creates one operation from all selected shapes
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I click "New operation from all selected"
    Then the document has 2 operations
    And the last operation has members 1, 2

  @UI-04
  Scenario: the plus menu offers each, all and by colour
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I open the "More new operation actions" menu
    Then "New operation: each selected" is on screen
    And "New operation: all selected" is on screen
    And "New operation by colour" is on screen
    When I choose "New operation: each selected"
    Then the document has 3 operations

  @UI-06
  Scenario: reordering with the keyboard changes the pass order
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I hammer the geometry row "inner"
    And I select operation 2
    And I press Ctrl+[
    Then operation 1 has members 1, 2
    And operation 2 has members 1, 2, 3

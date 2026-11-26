@GEO-06
Feature: Operation assignment

  Scenario: removing a shape's membership through the tag overflow
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    And an operation "Cut 2" over shapes 1, 2, 3
    When I remove shape #1 from the last operation
    Then the last operation has members 2, 3

  Scenario: geometry has no per-operation filter
    Given a nested fixture with groups "outer" and "inner"
    When I open the settings for operation 1
    Then "Show cut in geometry view" is not on screen

  @UI-03
  Scenario: assigning the selection to an existing operation
    Given the workspace is open
    And a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    And a second operation "Cut 2" over all shapes
    When shape 3 is selected
    And I click "Add to operation"
    Then "Choose an operation" is on screen
    When I click "Add to Cut 2"
    Then operation 2 has members 1, 2, 3

  @UI-01
  Scenario: adding geometry requires choosing its operation
    Given a document with 1 shape
    When I click the "Add shape" button
    Then "Choose an operation" is on screen

  @OP-07
  Scenario: the operations footer adds the selected geometry to the current operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1
    And shape 2 is selected
    When I select operation 2
    And I click "Add selected to current operation"
    Then operation 2 has members 1, 2

  @OP-07
  Scenario: the chevron adds all selected geometry to the current operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1
    And shapes 2, 3 are selected
    When I select operation 2
    And I open the "More new operation actions" menu
    And I choose "All selected to current operation"
    Then operation 2 has members 1, 2, 3

  @OP-07
  Scenario: dragging a geometry row onto an operation adds it
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1
    And all groups are expanded
    When I drag the geometry row "#2" onto the operation row "Cut 2"
    Then operation 2 has members 1, 2

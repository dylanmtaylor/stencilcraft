@GEO-06
Feature: Operation assignment

  Scenario: removing a shape's membership through the tag overflow
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I hammer the geometry row "outer"
    And I remove shape #1 from the last operation
    Then the last operation has members 2, 3

  Scenario: filtering an operation hides its shapes from the geometry view only
    Given a document with 2 shapes
    And the unassigned group is expanded
    When I hammer the geometry row "#1"
    And I hide every operation of shape #2 from the geometry view
    Then the geometry panel does not show "#2, #1"
    And the geometry panel shows "#1, #1 #2"
    And the document has 2 operations
    And operation 1 is visible

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

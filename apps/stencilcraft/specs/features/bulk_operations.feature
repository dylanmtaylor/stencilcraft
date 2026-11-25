@GEO-07
Feature: Bulk operation creation

  Scenario: under the threshold one operation covers all selected shapes
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I click "New operation: all selected"
    Then "operations?" is not on screen
    And the document has 2 operations
    And the last operation has members 1, 2

  Scenario: over the threshold asks before creating one operation per shape
    Given a nested fixture with groups "outer" and "inner"
    And the bulk confirm threshold is 2
    And shapes 1, 2, 3 are selected
    When I click "New operation: each selected"
    Then "Create 3 operations?" is on screen
    And the document has 1 operation
    When I click "Continue"
    Then the document has 4 operations

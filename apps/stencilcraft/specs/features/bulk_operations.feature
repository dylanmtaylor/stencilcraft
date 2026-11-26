@GEO-07
Feature: Bulk operation creation

  Scenario: under the threshold one operation covers all selected shapes
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I click "New operation from all selected"
    Then "operations?" is not on screen
    And the document has 2 operations
    And the last operation has members 1, 2

  Scenario: the plus menu offers an appearance popup
    Given a nested fixture with groups "outer" and "inner"
    And shape 1 has stroke "#ff0000"
    And shape 2 has stroke "#00ff00"
    When I open the "More new operation actions" menu
    And I choose "New operation by appearance"
    Then "Stroke" is on screen
    And "Fill" is on screen
    And "Both" is on screen
    When I choose the appearance value "Stroke #ff0000"
    Then the selected shapes are 1
    And the document has 2 operations
    And the last operation has members 1

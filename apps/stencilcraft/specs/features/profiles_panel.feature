@OP-03
Feature: Operation profiles

  Scenario: the profiles panel lists the base profiles
    Given the profiles panel is open
    Then "Profile Cut" is on screen
    And "Profile Score" is on screen
    And "Profile Engrave" is on screen
    And "Profile Guide" is not on screen
    And "No operation profiles yet" is not on screen
    And "New profile" is not on screen

  @OP-03
  Scenario: a profile row links the selected operation to its defaults
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I select operation 2
    And I click "Profile Score"
    Then operation 2 has kind "score"
    And the operations panel shows "Profile Score"

  @OP-03
  Scenario: a profile row unlinks and relinks the selected operation
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I select operation 2
    And I click "Profile Score"
    Then the operations panel shows "Profile Score"
    When I click "Unlink Profile Score"
    Then the operations panel shows "Profile none"
    When I click "Link Profile Score"
    Then the operations panel shows "Profile Score"

  @OP-03
  Scenario: the cog edits a profile's parameters
    Given the profiles panel is open
    When I click "Profile settings Cut"
    Then "Power" is on screen
    When I click "Close"
    Then "Power" is not on screen

  @OP-03
  Scenario: extracting profiles adds an operation's settings
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the settings for operation 2
    And I set the power to 42
    And I click "Close"
    And I click "Extract profiles"
    Then "Profile Cut 2" is on screen

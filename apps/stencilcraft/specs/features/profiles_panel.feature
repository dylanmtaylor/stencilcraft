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
    And I click "Link anyway"
    Then operation 2 has kind "score"
    And operation 2 has colour "#f5a524"
    And the operations panel shows "Profile Score"

  @OP-03
  Scenario: a profile row unlinks and relinks the selected operation
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I select operation 2
    And I click "Profile Score"
    And I click "Link anyway"
    Then the operations panel shows "Profile Score"
    When I click the link glyph "Unlink Profile Score"
    Then the operations panel shows "Profile none"
    When I click "Profile Score"
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

  @OP-03
  Scenario: a profile row shows how many operations are linked to it
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I select operation 1
    And I click "Profile Score"
    And I click "Link anyway"
    And I select operation 2
    And I click "Profile Score"
    And I click "Link anyway"
    Then "2 linked to Profile Score" is on screen

  @OP-03
  Scenario: a profile row separates its description from the name button
    Given the profiles panel is open
    Then the profile row for "Score" shows a description

  @OP-03
  Scenario: a profile's colour well edits the profile colour
    Given the profiles panel is open
    When I click the colour well "Profile colour Cut"
    And I set the picker colour to "#ff0000"
    And I click the colour well "Profile colour Cut"
    Then the profile "Cut" has colour "#ff0000"

  @OP-03
  Scenario: recolouring a profile recolours its linked operations
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I select operation 2
    And I click "Profile Score"
    And I click "Link anyway"
    And I click the colour well "Profile colour Score"
    And I set the picker colour to "#00ff00"
    And I click the colour well "Profile colour Score"
    Then the profile "Score" has colour "#00ff00"
    And operation 2 has colour "#00ff00"

  @OP-03
  Scenario: applying a profile with different settings asks first
    Given the operations and profiles panels are open
    And a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the settings for operation 2
    And I set the power to 40
    And I click "Close"
    And I select operation 2
    And I click "Profile Score"
    Then "1 operation differs. Link anyway?" is on screen
    When I click "Link anyway"
    Then operation 2 has kind "score"

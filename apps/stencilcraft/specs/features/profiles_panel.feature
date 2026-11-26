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

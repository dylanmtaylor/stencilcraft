@OP-03
Feature: Operation profiles

  Scenario: the profiles panel is an empty stub
    Given the profiles panel is open
    Then "No operation profiles yet" is on screen
    And "New profile" is not on screen

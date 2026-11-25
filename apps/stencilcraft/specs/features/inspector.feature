@UI-07
Feature: Inspector

  Background:
    Given the workspace is open

  Scenario: the inspector shows the effective operation of the selected shape
    Given a nested fixture with groups "outer" and "inner"
    And shape 1 is selected
    Then the inspector shows the header "Shape #1 - Path"
    And the inspector shows the operation "cut"

  @gap @UI-07
  Scenario: selecting an operation shows it in the inspector
    Given a nested fixture with groups "outer" and "inner"
    When I click the first operation row
    Then the inspector shows the operation "cut"

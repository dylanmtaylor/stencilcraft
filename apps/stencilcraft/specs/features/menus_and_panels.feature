@UI-02
Feature: Menus and panels

  Background:
    Given the menu bar is open

  Scenario: the Panels menu offers the Geometry and Operations panels
    When I open the "Panels" menu
    Then "Geometry" is on screen
    And "Operations" is on screen

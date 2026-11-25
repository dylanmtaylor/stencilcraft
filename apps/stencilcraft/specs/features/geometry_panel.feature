@GEO-04
Feature: Geometry panel

  Scenario: nested groups appear in the geometry tree
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    Then the geometry panel shows "outer"
    And the geometry panel shows "inner"
    And the geometry panel shows "#1"
    And the geometry panel shows "Unassigned"
    And the document has 2 groups

  Scenario: clicking a shape row selects that shape
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I click the geometry row "#2, #1"
    Then the selected shapes are 2

  Scenario: clicking a group row deep-selects every member
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I click the geometry row "inner, 2 shapes"
    Then the selected shapes are 1, 2

  Scenario: ungrouping dissolves the selected group
    Given a nested fixture with groups "outer" and "inner"
    And group 2 is selected in the geometry panel
    When I click "Ungroup selected"
    Then the document has 1 group

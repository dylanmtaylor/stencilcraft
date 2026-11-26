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
    When I click the geometry row "#2"
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

  @LY-01
  Scenario: operation tags are contextual pills
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    When I click "Operation cut on #1"
    Then "Remove #1 from cut" is on screen
    And "Operations of #1" is not on screen

  @LY-01
  Scenario: geometry offers no operation creation
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    Then "New operation from outer" is not on screen
    And "New operation from #1" is not on screen

  @LY-01
  Scenario: geometry rows show no operation ids
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    And an operation "Cut 2" over shapes 1
    Then the geometry panel does not show "#1 #2"

  @LY-01
  Scenario: the tag overflow opens per-operation submenus
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    And an operation "Cut 2" over shapes 1
    And an operation "Cut 3" over shapes 1
    And an operation "Cut 4" over shapes 1
    When I click "More operations for #1"
    Then the menu item "Cut 4" is on screen

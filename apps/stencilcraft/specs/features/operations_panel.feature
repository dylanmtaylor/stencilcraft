@GEO-05
Feature: Operations panel

  @UI-04
  Scenario: reordering an operation changes its pass order
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I move operation 2 up
    Then operation 1 has members 1, 2, 3
    And the document has 2 operations

  @UI-05
  Scenario: editing an operation's settings
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the settings for operation 2
    Then "Power" is on screen
    When I set the power to 40
    And I click "Close"
    Then operation 2 has power 40

  @UI-05
  Scenario: changing an operation kind
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I set operation 2 to kind "engrave"
    Then operation 2 has kind "engrave"

  @UI-05
  Scenario: the operations list has a visible lock toggle
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I toggle the lock on operation 2
    Then operation 2 is locked

  @UI-05
  Scenario: deleting an operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I delete operation 2
    Then the document has 1 operation

  @UI-04
  Scenario: expanding an operation lists its member shapes
    Given a nested fixture with groups "outer" and "inner"
    When I expand operation 1
    Then the operations panel shows "#1"
    And the operations panel shows "#2"
    And the operations panel shows "#3"

  @UI-04
  Scenario: removing a member from the expanded operation row
    Given a nested fixture with groups "outer" and "inner"
    When I expand operation 1
    And I remove member #1 from operation 1
    Then operation 1 has members 2, 3

  @UI-04
  Scenario: double-clicking a member row selects that member
    Given a nested fixture with groups "outer" and "inner"
    When I expand operation 1
    And I double-click member 2 of operation 1
    Then the selected shapes are 2

  @UI-05
  Scenario: the settings popover has no view filter
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the settings for operation 2
    Then "Kind" is on screen
    And "Show Cut 2 in geometry view" is not on screen

  @UI-05
  Scenario: the colour popup offers 40 palette colours
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the colour of operation 2
    Then "Palette colour 1" is on screen
    And "Palette colour 40" is on screen
    And "Hex" is on screen
    When I click the palette colour 1
    And I close the colour picker
    Then operation 2 has colour "#000000"

  @UI-05
  Scenario: the colour swatch sets the operation colour
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I open the colour of operation 2
    And I set the colour to "#ff0000"
    And I close the colour picker
    Then operation 2 has colour "#ff0000"

  @UI-05
  Scenario: the trash button deletes an operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1, 2, 3
    When I click "Delete operation Cut 2"
    Then the document has 1 operation

  @UI-04
  Scenario: the plus button creates one operation from all selected shapes
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I click "New operation from all selected"
    Then the document has 2 operations
    And the last operation has members 1, 2

  @UI-04
  Scenario: the plus menu offers each, all and appearance
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I open the "More new operation actions" menu
    Then "New operation: each selected" is on screen
    And "New operation: all selected" is on screen
    And "New operation by appearance" is on screen
    When I choose "New operation: each selected"
    Then the document has 3 operations

  @OP-03
  Scenario: a new operation links to Cut and editing settings unlinks it
    Given a nested fixture with groups "outer" and "inner"
    And shapes 1, 2 are selected
    When I click "New operation from all selected"
    Then the operations panel shows "Profile Cut"
    When I open the settings for operation 2
    And I set the power to 40
    And I click "Close"
    Then the operations panel shows "Profile none"

  @UI-06
  Scenario: reordering with the keyboard changes the pass order
    Given a nested fixture with groups "outer" and "inner"
    And all groups are expanded
    And an operation "Cut 2" over shapes 1, 2
    And I select operation 2
    And I press Ctrl+[
    Then operation 1 has members 1, 2
    And operation 2 has members 1, 2, 3

  @OP-03
  Scenario: a linked operation shows the profile chain
    Given a nested fixture with groups "outer" and "inner"
    Then the operations panel shows "Profile Cut"
    And a label containing "Unlink Profile Cut" is on screen

  @OP-03
  Scenario: breaking the chain unlinks the operation
    Given a nested fixture with groups "outer" and "inner"
    When I click the link glyph "Unlink Profile Cut"
    Then the operations panel shows "Profile none"
    And a label containing "Unlink Profile Cut" is not on screen

  @UI-04
  Scenario: dragging a member row moves it to another operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1
    When I expand operation 1
    And I drag member 3 of operation 1 onto operation 2
    Then operation 1 has members 1, 2
    And operation 2 has members 1, 3

  @UI-04
  Scenario: shift-dragging a member row adds it to another operation
    Given a nested fixture with groups "outer" and "inner"
    And an operation "Cut 2" over shapes 1
    When I expand operation 1
    And I shift-drag member 3 of operation 1 onto operation 2
    Then operation 1 has members 1, 2, 3
    And operation 2 has members 1, 3

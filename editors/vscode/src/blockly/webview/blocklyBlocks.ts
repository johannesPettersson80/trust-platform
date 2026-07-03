/**
 * Custom Blockly block definitions for PLC programming
 */
import * as Blockly from 'blockly';

export function registerPLCBlocks() {
  // IO Digital Write Block
  Blockly.Blocks['io_digital_write'] = {
    init: function() {
      this.appendValueInput('VALUE')
        .setCheck('Boolean')
        .appendField('Write')
        .appendField(new Blockly.FieldTextInput('%QX0.0'), 'ADDRESS');
      this.setPreviousStatement(true, null);
      this.setNextStatement(true, null);
      this.setStyle('io_blocks');
      this.setTooltip('Write digital output');
      this.setHelpUrl('');
    }
  };

  // IO Digital Read Block
  Blockly.Blocks['io_digital_read'] = {
    init: function() {
      this.appendDummyInput()
        .appendField('Read')
        .appendField(new Blockly.FieldTextInput('%IX0.0'), 'ADDRESS');
      this.setOutput(true, 'Boolean');
      this.setStyle('io_blocks');
      this.setTooltip('Read digital input');
      this.setHelpUrl('');
    }
  };

  // Comment Block
  Blockly.Blocks['comment'] = {
    init: function() {
      this.appendDummyInput()
        .appendField('--')
        .appendField(new Blockly.FieldTextInput('comment'), 'TEXT');
      this.setPreviousStatement(true, null);
      this.setNextStatement(true, null);
      this.setStyle('comment_blocks');
      this.setTooltip('Add a comment');
      this.setHelpUrl('');
    }
  };

  // Timer TON Block
  Blockly.Blocks['timer_ton'] = {
    init: function() {
      this.appendValueInput('PT')
        .setCheck('Number')
        .appendField('TON')
        .appendField(new Blockly.FieldTextInput('timer1'), 'NAME')
        .appendField('PT');
      this.appendValueInput('IN')
        .setCheck('Boolean')
        .appendField('IN');
      this.appendDummyInput()
        .appendField('Q')
        .appendField(new Blockly.FieldVariable('timer1_Q'), 'Q');
      this.setPreviousStatement(true, null);
      this.setNextStatement(true, null);
      this.setStyle('timer_blocks');
      this.setTooltip('On-Delay Timer');
this.setHelpUrl('');
    }
  };

  // Counter CTU Block
  Blockly.Blocks['counter_ctu'] = {
    init: function() {
      this.appendValueInput('PV')
        .setCheck('Number')
        .appendField('CTU')
        .appendField(new Blockly.FieldTextInput('counter1'), 'NAME')
        .appendField('PV');
      this.appendValueInput('CU')
        .setCheck('Boolean')
        .appendField('CU');
      this.appendValueInput('R')
        .setCheck('Boolean')
        .appendField('R');
      this.appendDummyInput()
        .appendField('Q')
        .appendField(new Blockly.FieldVariable('counter1_Q'), 'Q');
      this.setPreviousStatement(true, null);
      this.setNextStatement(true, null);
      this.setStyle('counter_blocks');
      this.setTooltip('Up Counter');
      this.setHelpUrl('');
    }
  };

  // Math Compare Block (extended for PLC)
  if (!Blockly.Blocks['logic_compare_plc']) {
    Blockly.Blocks['logic_compare_plc'] = {
      init: function() {
        const OPERATORS = [
          ['=', 'EQ'],
          ['≠', 'NEQ'],
          ['<', 'LT'],
          ['≤', 'LTE'],
          ['>', 'GT'],
          ['≥', 'GTE']
        ];
        this.setHelpUrl('');
        this.setStyle('logic_blocks');
        this.setOutput(true, 'Boolean');
        this.appendValueInput('A');
        this.appendValueInput('B')
          .appendField(new Blockly.FieldDropdown(OPERATORS), 'OP');
        this.setInputsInline(true);
        this.setTooltip('Compare two values');
      }
    };
  }
}

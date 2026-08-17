'use strict';
const Segment = require('../index');
function useDefault(segment) { return segment; }
function getDefaultModList() { return []; }
module.exports = { Segment, useDefault, getDefaultModList, default: Segment };

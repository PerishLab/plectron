const Test262Error = function (message) {
    if (!(this instanceof Test262Error)) {
        return new Test262Error(message);
    }
    this.message = message || "";
};
Test262Error.prototype.toString = function () {
    return "Test262Error: " + this.message;
};
Test262Error.thrower = function (message) {
    throw new Test262Error(message);
};
Test262Error;

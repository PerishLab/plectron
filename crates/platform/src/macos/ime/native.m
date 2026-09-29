#import <AppKit/AppKit.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static int trace_enabled(void) {
    static int enabled = -1;
    if (enabled < 0) enabled = getenv("PLECTRON_IME_TRACE") != NULL;
    return enabled;
}

static double trace_time(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return now.tv_sec + now.tv_nsec / 1000000000.0;
}

#define TRACE(...) do { if (trace_enabled()) { fprintf(stderr, "[ime %.6f] ", trace_time()); fprintf(stderr, __VA_ARGS__); fputc('\n', stderr); } } while (0)

typedef struct PlectronImeEvent {
    uint32_t kind;
    uint64_t id;
    char *text;
    size_t start;
    size_t end;
} PlectronImeEvent;

typedef struct PlectronImeNode {
    PlectronImeEvent event;
    struct PlectronImeNode *next;
} PlectronImeNode;

void plectron_window_capture(void *view, void *event);

@interface PlectronImeView : NSView <NSTextInputClient> {
    NSString *marked;
    NSRange selected;
    uint64_t serial;
    uint64_t active;
    size_t target;
    BOOL focused;
    NSString *document;
    NSRange insertion;
    NSRect caret;
    PlectronImeNode *head;
    PlectronImeNode *tail;
}
- (void)emit:(uint32_t)kind text:(NSString *)text selection:(NSRange)range;
- (BOOL)pop:(PlectronImeEvent *)output;
- (void)cancel;
- (void)context:(size_t)nextTarget focused:(BOOL)nextFocused text:(const char *)text start:(size_t)start end:(size_t)end
                x:(double)x y:(double)y height:(double)height;
@end

@implementation PlectronImeView
- (BOOL)acceptsFirstResponder { return YES; }

- (void)dealloc {
    [marked release];
    [document release];
    while (head) {
        PlectronImeNode *next = head->next;
        free(head->event.text);
        free(head);
        head = next;
    }
    [super dealloc];
}

- (void)keyDown:(NSEvent *)event {
    BOOL composing = active != 0;
    TRACE("keyDown code=%hu composing=%d selected=%lu:%lu", [event keyCode], composing,
          (unsigned long)[self selectedRange].location, (unsigned long)[self selectedRange].length);
    if (!composing) plectron_window_capture([self superview], event);
    BOOL handled = [[self inputContext] handleEvent:event];
    if (composing && !handled) plectron_window_capture([self superview], event);
    TRACE("keyDone code=%hu handled=%d active=%llu", [event keyCode], handled, active);
}

- (void)keyUp:(NSEvent *)event { (void)event; }
- (void)flagsChanged:(NSEvent *)event { (void)event; }

- (void)emit:(uint32_t)kind text:(NSString *)text selection:(NSRange)range {
    PlectronImeNode *node = calloc(1, sizeof(PlectronImeNode));
    if (!node) return;
    node->event.kind = kind;
    node->event.id = active;
    node->event.text = strdup([text UTF8String]);
    node->event.start = range.location;
    node->event.end = range.location + range.length;
    if (!node->event.text) { free(node); return; }
    if (tail) tail->next = node;
    else head = node;
    tail = node;
    TRACE("emit kind=%u id=%llu bytes=%zu range=%zu:%zu", kind, active,
          strlen(node->event.text), node->event.start, node->event.end);
}

- (BOOL)pop:(PlectronImeEvent *)output {
    if (!head) return NO;
    PlectronImeNode *node = head;
    head = node->next;
    if (!head) tail = NULL;
    *output = node->event;
    free(node);
    TRACE("pop kind=%u id=%llu bytes=%zu", output->kind, output->id, strlen(output->text));
    return YES;
}

- (void)cancel {
    if (!active) return;
    TRACE("cancel id=%llu", active);
    [self emit:3 text:@"" selection:NSMakeRange(0, 0)];
    active = 0;
    [marked release];
    marked = nil;
    [[self inputContext] discardMarkedText];
}

- (void)context:(size_t)nextTarget focused:(BOOL)nextFocused text:(const char *)text start:(size_t)start end:(size_t)end
                x:(double)x y:(double)y height:(double)height {
    TRACE("context target=%zu focused=%d bytes=%zu range=%zu:%zu caret=%.0f,%.0f,%.0f active=%llu",
          nextTarget, nextFocused, text ? strlen(text) : 0, start, end, x, y, height, active);
    if (active && (!nextFocused || !focused || target != nextTarget)) [self cancel];
    target = nextTarget;
    focused = nextFocused;
    [document release];
    document = text ? [[NSString alloc] initWithUTF8String:text] : nil;
    if (!active) insertion = NSMakeRange(start, end - start);
    caret = NSMakeRect(x, y, 1, height);
}

- (BOOL)hasMarkedText { return active != 0; }
- (NSRange)markedRange {
    return active ? NSMakeRange(insertion.location, [marked length]) : NSMakeRange(NSNotFound, 0);
}
- (NSRange)selectedRange { return active ? selected : insertion; }
- (NSArray *)validAttributesForMarkedText { return [NSArray array]; }
- (NSAttributedString *)attributedSubstringForProposedRange:(NSRange)range actualRange:(NSRangePointer)actual {
    NSString *text = document;
    if (active && document && insertion.location <= [document length] && NSMaxRange(insertion) <= [document length]) {
        text = [NSString stringWithFormat:@"%@%@%@",
                [document substringToIndex:insertion.location], marked,
                [document substringFromIndex:NSMaxRange(insertion)]];
    }
    if (!text) return nil;
    if (range.location > [text length]) return nil;
    NSRange local = NSMakeRange(range.location, MIN(range.length, [text length] - range.location));
    if (actual) *actual = local;
    return [[[NSAttributedString alloc] initWithString:[text substringWithRange:local]] autorelease];
}
- (NSUInteger)characterIndexForPoint:(NSPoint)point { return selected.location; }
- (NSRect)firstRectForCharacterRange:(NSRange)range actualRange:(NSRangePointer)actual {
    if (actual) *actual = NSMakeRange(MIN(range.location, [self selectedRange].location), 0);
    NSView *host = [self superview];
    NSRect local = NSMakeRect(caret.origin.x, [host bounds].size.height - caret.origin.y - caret.size.height, 1, caret.size.height);
    NSRect window = [host convertRect:local toView:nil];
    return [[self window] convertRectToScreen:window];
}
- (void)doCommandBySelector:(SEL)selector { (void)selector; }

- (void)setMarkedText:(id)value selectedRange:(NSRange)range replacementRange:(NSRange)replacement {
    (void)replacement;
    NSString *text = [value isKindOfClass:[NSAttributedString class]] ? [value string] : value;
    TRACE("marked bytes=%zu select=%lu:%lu replace=%lu:%lu", strlen([text UTF8String]),
          (unsigned long)range.location, (unsigned long)range.length,
          (unsigned long)replacement.location, (unsigned long)replacement.length);
    if (!active) active = ++serial;
    [marked release];
    marked = [text copy];
    NSUInteger length = [marked length];
    NSUInteger start = MIN(range.location, length);
    NSUInteger end = start + MIN(range.length, length - start);
    selected = NSMakeRange(insertion.location + start, end - start);
    [self emit:1 text:marked selection:NSMakeRange(start, end - start)];
}

- (void)insertText:(id)value replacementRange:(NSRange)replacement {
    (void)replacement;
    NSString *text = [value isKindOfClass:[NSAttributedString class]] ? [value string] : value;
    TRACE("insert bytes=%zu replace=%lu:%lu active=%llu", strlen([text UTF8String]),
          (unsigned long)replacement.location, (unsigned long)replacement.length, active);
    [self emit:active ? 2 : 4 text:text selection:NSMakeRange(0, 0)];
    active = 0;
    [marked release];
    marked = nil;
}

- (void)unmarkText {
    TRACE("unmark active=%llu", active);
    if (active) [self emit:2 text:marked selection:NSMakeRange(0, 0)];
    active = 0;
    [marked release];
    marked = nil;
}
@end

void *plectron_ime_attach(void *parent) {
    NSView *host = (NSView *)parent;
    PlectronImeView *view = [[PlectronImeView alloc] initWithFrame:NSMakeRect(0, 0, 1, 1)];
    [host addSubview:view];
    [[host window] makeFirstResponder:view];
    return view;
}

int plectron_ime_pop(void *handle, PlectronImeEvent *event) {
    return [(PlectronImeView *)handle pop:event];
}

void plectron_ime_cancel(void *handle) { [(PlectronImeView *)handle cancel]; }

int plectron_ime_active(void *handle) {
    return [[(PlectronImeView *)handle window] isKeyWindow];
}

int plectron_ime_trace_enabled(void) { return trace_enabled(); }

void plectron_ime_context(void *handle, size_t target, int focused, const char *text, size_t start, size_t end,
                         double x, double y, double height) {
    [(PlectronImeView *)handle context:target focused:focused text:text start:start end:end x:x y:y height:height];
}

void plectron_ime_detach(void *handle) {
    PlectronImeView *view = (PlectronImeView *)handle;
    [[view window] makeFirstResponder:[view window]];
    [view removeFromSuperview];
    [view release];
}

void plectron_ime_free(char *text) { free(text); }

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct PlectronWindowEvent {
    uint32_t kind;
    uint16_t code;
    uint8_t repeat;
    uint64_t flags;
    double x;
    double y;
    double delta;
} PlectronWindowEvent;

typedef struct PlectronWindowNode {
    PlectronWindowEvent event;
    struct PlectronWindowNode *next;
} PlectronWindowNode;

typedef struct PlectronWindowState PlectronWindowState;

@interface PlectronWindow : NSWindow <NSWindowDelegate> {
    PlectronWindowState *owner;
}
- (void)bind:(PlectronWindowState *)state;
@end

@interface PlectronWindowView : NSView {
    PlectronWindowState *owner;
    NSTrackingArea *tracking;
}
- (id)initWithFrame:(NSRect)frame owner:(PlectronWindowState *)state;
- (void)capture:(NSEvent *)event;
@end

struct PlectronWindowState {
    PlectronWindow *window;
    PlectronWindowView *view;
    PlectronWindowNode *head;
    PlectronWindowNode *tail;
    size_t length;
    int overflow;
};

static void enqueue(PlectronWindowState *state, PlectronWindowEvent event) {
    if (state->length >= 512) {
        state->overflow = 1;
        return;
    }
    PlectronWindowNode *node = calloc(1, sizeof(PlectronWindowNode));
    if (!node) {
        state->overflow = 1;
        return;
    }
    node->event = event;
    if (state->tail) state->tail->next = node;
    else state->head = node;
    state->tail = node;
    state->length += 1;
}

static NSPoint position(PlectronWindowView *view, NSEvent *event) {
    return [view convertPoint:[event locationInWindow] fromView:nil];
}

@implementation PlectronWindow
- (void)bind:(PlectronWindowState *)state {
    owner = state;
    [self setDelegate:self];
}
- (BOOL)windowShouldClose:(id)sender {
    (void)sender;
    enqueue(owner, (PlectronWindowEvent){ .kind = 1 });
    return NO;
}
@end

@implementation PlectronWindowView
- (id)initWithFrame:(NSRect)frame owner:(PlectronWindowState *)state {
    self = [super initWithFrame:frame];
    if (self) {
        owner = state;
        [self setWantsLayer:YES];
    }
    return self;
}
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)isFlipped { return YES; }
- (void)updateTrackingAreas {
    if (tracking) {
        [self removeTrackingArea:tracking];
        [tracking release];
    }
    tracking = [[NSTrackingArea alloc]
        initWithRect:NSZeroRect
        options:NSTrackingMouseMoved | NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect
        owner:self
        userInfo:nil];
    [self addTrackingArea:tracking];
    [super updateTrackingAreas];
}
- (void)dealloc {
    if (tracking) {
        [self removeTrackingArea:tracking];
        [tracking release];
    }
    [super dealloc];
}
- (void)move:(NSEvent *)event {
    NSPoint point = position(self, event);
    enqueue(owner, (PlectronWindowEvent){ .kind = 2, .x = point.x, .y = point.y });
}
- (void)mouseMoved:(NSEvent *)event { [self move:event]; }
- (void)mouseDragged:(NSEvent *)event { [self move:event]; }
- (void)mouseDown:(NSEvent *)event {
    NSPoint point = position(self, event);
    enqueue(owner, (PlectronWindowEvent){ .kind = 3, .x = point.x, .y = point.y });
}
- (void)mouseUp:(NSEvent *)event {
    NSPoint point = position(self, event);
    enqueue(owner, (PlectronWindowEvent){ .kind = 4, .x = point.x, .y = point.y });
}
- (void)scrollWheel:(NSEvent *)event {
    NSPoint point = position(self, event);
    enqueue(owner, (PlectronWindowEvent){
        .kind = 5,
        .code = [event hasPreciseScrollingDeltas] ? 1 : 0,
        .x = point.x,
        .y = point.y,
        .delta = [event scrollingDeltaY],
    });
}
- (void)capture:(NSEvent *)event {
    enqueue(owner, (PlectronWindowEvent){
        .kind = 6,
        .code = [event keyCode],
        .repeat = [event isARepeat],
        .flags = [event modifierFlags],
    });
}
- (void)keyDown:(NSEvent *)event { [self capture:event]; }
@end

void *plectron_window_create(const char *title, size_t width, size_t height) {
    @autoreleasepool {
        PlectronWindowState *state = calloc(1, sizeof(PlectronWindowState));
        if (!state) return NULL;
        NSRect frame = NSMakeRect(0, 0, width, height);
        NSWindowStyleMask style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable;
        state->window = [[PlectronWindow alloc]
            initWithContentRect:frame
            styleMask:style
            backing:NSBackingStoreBuffered
            defer:NO];
        if (!state->window) {
            free(state);
            return NULL;
        }
        state->view = [[PlectronWindowView alloc] initWithFrame:frame owner:state];
        if (!state->view) {
            [state->window release];
            free(state);
            return NULL;
        }
        [state->window bind:state];
        [state->window setReleasedWhenClosed:NO];
        [state->window setTitle:[NSString stringWithUTF8String:title]];
        [state->window setContentView:state->view];
        [state->window setAcceptsMouseMovedEvents:YES];
        [state->window center];
        return state;
    }
}

void plectron_window_show(void *handle) {
    PlectronWindowState *state = handle;
    [state->window orderFront:nil];
}

void plectron_window_key(void *handle) {
    PlectronWindowState *state = handle;
    [state->window makeKeyWindow];
}

void plectron_window_title(void *handle, const char *title) {
    @autoreleasepool {
        PlectronWindowState *state = handle;
        [state->window setTitle:[NSString stringWithUTF8String:title]];
    }
}

void *plectron_window_view(void *handle) {
    return ((PlectronWindowState *)handle)->view;
}

double plectron_window_scale(void *handle) {
    return [((PlectronWindowState *)handle)->window backingScaleFactor];
}

int plectron_window_pop(void *handle, PlectronWindowEvent *event) {
    PlectronWindowState *state = handle;
    if (state->overflow) {
        state->overflow = 0;
        event->kind = 7;
        return 1;
    }
    if (!state->head) return 0;
    PlectronWindowNode *node = state->head;
    state->head = node->next;
    if (!state->head) state->tail = NULL;
    state->length -= 1;
    *event = node->event;
    free(node);
    return 1;
}

int plectron_window_present(void *handle, const uint32_t *pixels, size_t width, size_t height) {
    if (!pixels || !width || !height) return 0;
    size_t bytes = width * height * sizeof(uint32_t);
    CFDataRef data = CFDataCreate(NULL, (const UInt8 *)pixels, bytes);
    if (!data) return 0;
    CGDataProviderRef provider = CGDataProviderCreateWithCFData(data);
    CFRelease(data);
    if (!provider) return 0;
    CGColorSpaceRef colors = CGColorSpaceCreateDeviceRGB();
    CGBitmapInfo layout = kCGBitmapByteOrder32Little | kCGImageAlphaNoneSkipFirst;
    CGImageRef image = CGImageCreate(width, height, 8, 32, width * sizeof(uint32_t), colors,
                                     layout, provider, NULL, false, kCGRenderingIntentDefault);
    CGColorSpaceRelease(colors);
    CGDataProviderRelease(provider);
    if (!image) return 0;
    PlectronWindowState *state = handle;
    CALayer *layer = [state->view layer];
    [layer setContentsScale:[state->window backingScaleFactor]];
    [layer setContentsGravity:kCAGravityResize];
    [layer setContents:(id)image];
    CGImageRelease(image);
    return 1;
}

void plectron_window_close(void *handle) {
    PlectronWindowState *state = handle;
    [state->window setDelegate:nil];
    [state->window close];
}

void plectron_window_destroy(void *handle) {
    PlectronWindowState *state = handle;
    if (!state) return;
    plectron_window_close(handle);
    [state->window setContentView:nil];
    [state->view release];
    [state->window release];
    while (state->head) {
        PlectronWindowNode *next = state->head->next;
        free(state->head);
        state->head = next;
    }
    free(state);
}

void plectron_window_capture(void *view, void *event) {
    [(PlectronWindowView *)view capture:(NSEvent *)event];
}

void plectron_application_poll(void) {
    NSAutoreleasePool *pool = [[NSAutoreleasePool alloc] init];
    NSEvent *event;
    do {
        event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                   untilDate:[NSDate distantPast]
                                      inMode:NSDefaultRunLoopMode
                                     dequeue:YES];
        if (event) [NSApp sendEvent:event];
    } while (event);
    [pool drain];
}

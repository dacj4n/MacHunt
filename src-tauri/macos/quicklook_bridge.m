#import <Cocoa/Cocoa.h>
#import <QuickLookUI/QuickLookUI.h>
#import <stdbool.h>
#import <stddef.h>

@interface MachuntQuickLookController : NSResponder <QLPreviewPanelDataSource, QLPreviewPanelDelegate>
@property(nonatomic, copy) NSArray<NSURL *> *urls;
@property(nonatomic, assign) NSInteger currentIndex;
@end

@implementation MachuntQuickLookController

+ (instancetype)sharedController {
  static MachuntQuickLookController *controller = nil;
  static dispatch_once_t onceToken;
  dispatch_once(&onceToken, ^{
    controller = [[MachuntQuickLookController alloc] init];
  });
  return controller;
}

- (instancetype)init {
  self = [super init];
  if (self) {
    _urls = @[];
    _currentIndex = 0;
  }
  return self;
}

- (BOOL)acceptsPreviewPanelControl:(QLPreviewPanel *)panel {
  (void)panel;
  return YES;
}

- (void)beginPreviewPanelControl:(QLPreviewPanel *)panel {
  panel.dataSource = self;
  panel.delegate = self;
}

- (void)endPreviewPanelControl:(QLPreviewPanel *)panel {
  (void)panel;
}

- (NSInteger)numberOfPreviewItemsInPreviewPanel:(QLPreviewPanel *)panel {
  (void)panel;
  return self.urls.count;
}

- (id<QLPreviewItem>)previewPanel:(QLPreviewPanel *)panel previewItemAtIndex:(NSInteger)index {
  (void)panel;
  NSInteger count = (NSInteger)self.urls.count;
  if (index < 0 || index >= count) {
    return nil;
  }
  return self.urls[index];
}

- (void)previewPanel:(QLPreviewPanel *)panel didChangeCurrentPreviewItem:(id<QLPreviewItem>)item {
  (void)panel;
  NSUInteger idx = [self.urls indexOfObjectIdenticalTo:(NSURL *)item];
  if (idx != NSNotFound) {
    self.currentIndex = (NSInteger)idx;
  }
}

@end

static NSArray<NSURL *> *build_urls(const char *const *paths, size_t len) {
  NSMutableArray<NSURL *> *urls = [NSMutableArray arrayWithCapacity:len];
  NSFileManager *fileManager = [NSFileManager defaultManager];

  for (size_t i = 0; i < len; i++) {
    const char *raw = paths[i];
    if (raw == NULL) {
      continue;
    }
    NSString *path = [NSString stringWithUTF8String:raw];
    if (path.length == 0) {
      continue;
    }
    BOOL isDirectory = NO;
    if (![fileManager fileExistsAtPath:path isDirectory:&isDirectory]) {
      continue;
    }
    (void)isDirectory;
    [urls addObject:[NSURL fileURLWithPath:path]];
  }

  return [urls copy];
}

bool open_quicklook(const char *const *paths, size_t len, size_t index) {
  @autoreleasepool {
    if (paths == NULL || len == 0) {
      return false;
    }

    NSArray<NSURL *> *urls = build_urls(paths, len);
    if (urls.count == 0) {
      return false;
    }

    __block bool opened = false;
    void (^openPanel)(void) = ^{
      MachuntQuickLookController *controller = [MachuntQuickLookController sharedController];
      controller.urls = urls;

      NSInteger initialIndex = (NSInteger)index;
      NSInteger count = (NSInteger)controller.urls.count;
      if (initialIndex < 0 || initialIndex >= count) {
        initialIndex = 0;
      }
      controller.currentIndex = initialIndex;

      [NSApp activateIgnoringOtherApps:YES];

      QLPreviewPanel *panel = [QLPreviewPanel sharedPreviewPanel];
      if (panel == nil) {
        opened = false;
        return;
      }

      panel.dataSource = controller;
      panel.delegate = controller;
      [panel reloadData];
      [panel setCurrentPreviewItemIndex:controller.currentIndex];
      [panel makeKeyAndOrderFront:nil];
      opened = true;
    };

    if ([NSThread isMainThread]) {
      openPanel();
    } else {
      dispatch_sync(dispatch_get_main_queue(), openPanel);
    }

    return opened;
  }
}

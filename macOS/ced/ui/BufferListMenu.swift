//
//  BufferListMenu.swift
//  ced
//
//  Created by Martin Chaine on 30/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class BufferListMenu: NSMenu, NSMenuDelegate {
    
    @IBOutlet var appDelegate: AppDelegate!
    
    required init(coder decoder: NSCoder) {
        super.init(coder: decoder)
        self.delegate = self
    }
    
    private func newItem(title: String) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: #selector(AppDelegate.selectBuffer), keyEquivalent: "")
        item.target = self.appDelegate
        return item
    }
    
    func populate() {
        self.removeAllItems()
        let context = self.appDelegate.focusedController().ced.context
        for buffer in context.bufferList.keys.sorted() {
            self.addItem(self.newItem(title: buffer))
        }
    }
    
    func menuWillOpen(_ menu: NSMenu) {
        self.populate()
    }

}

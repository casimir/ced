//
//  SessionListMenu.swift
//  ced
//
//  Created by Martin Chaine on 28/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class SessionListMenu: NSMenu, NSMenuDelegate {
    
    @IBOutlet var appDelegate: AppDelegate!
    
    required init(coder decoder: NSCoder) {
        super.init(coder: decoder)
        self.delegate = self
    }
    
    private func newItem(title: String, selector: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: selector, keyEquivalent: "")
        item.target = self.appDelegate
        return item
    }
    
    private func newCreateItem(title: String) -> NSMenuItem {
        return self.newItem(title: title, selector: #selector(AppDelegate.connectNewSession))
    }
    
    private func newConnectItem(title: String) -> NSMenuItem {
        return self.newItem(title: title, selector: #selector(AppDelegate.connectSession))
    }
    
    func populate() {
        self.removeAllItems()
        self.addItem(self.newCreateItem(title: "New session"))
        self.addItem(NSMenuItem.separator())
        for session in Ced.listSessions().sorted() {
            self.addItem(self.newConnectItem(title: session))
        }
    }
    
    func menuWillOpen(_ menu: NSMenu) {
        self.populate()
    }
    
}

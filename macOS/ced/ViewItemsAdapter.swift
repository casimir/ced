//
//  ViewItemsAdapter.swift
//  ced
//
//  Created by Martin Chaine on 24/10/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class ViewItemsAdapter: NSObject {
    
    var items: [[String: Any]] = [[String: Any]]() {
        didSet {
            tableView.reloadData()
            let index = IndexSet(0...(self.items.count - 1))
            tableView.noteHeightOfRows(withIndexesChanged: index) // TODO check if really changed
        }
    }
    
    private var tableView: NSTableView
    
    init(tableView: NSTableView) {
        self.tableView = tableView
        super.init()
        self.tableView.dataSource = self
        self.tableView.delegate = self
    }

}

extension ViewItemsAdapter: NSTableViewDelegate, NSTableViewDataSource {
    
    func numberOfRows(in tableView: NSTableView) -> Int {
        return items.count
    }
    
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let item = self.items[row]
        switch item["type"] as! String {
        case "Header":
            let buffer = item["buffer"] as! String
            let start = item["start"] as! uint
            let end = item["end"] as! uint
            return ViewItemHeaderView(buffer: buffer, range: (start, end))
        case "Lines":
            let lines = item["lines"] as! [String]
//            return ViewItemLinesView(lines: lines)
            let view = NSTextField(string: lines.joined(separator: "\n"))
            view.isEditable = false
            view.isBordered = false
            return view
        default:
            fatalError("invalid item type: \(item["type"] as! String)")
        }
    }
    
    func tableView(_ tableView: NSTableView, isGroupRow row: Int) -> Bool {
        return self.items[row]["type"] as! String == "Header"
    }
    
}

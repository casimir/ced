//
//  UIBinder.swift
//  ced
//
//  Created by Martin Chaine on 18/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class ConnectionController {

    var runningRequests: [RpcId: RpcRequest] = [:]
    
    var window: NSWindow!
    var viewItems: ViewItemsAdapter
    
    init(viewItems: ViewItemsAdapter) {
        self.viewItems = viewItems
    }
    
    func setWindowTitle(_ context: CedContext) {
        self.window?.title = "[\(context.session!)]"
    }
    
    func setView(_ context: CedContext) {
//        var colourFlag = true;
//        self.view.translatesAutoresizingMaskIntoConstraints = false
//        let column = self.viewItems.tableColumns[0]
        for item in context.view {
//            let text = NSTextField(string: String(describing: item))
//            text.isEditable = false
//            text.isBordered = false
//            text.backgroundColor = colourFlag ? NSColor.red : NSColor.blue
//            colourFlag = !colourFlag
//            self.viewItems.addArrangedSubview(text)
            
            
//            text.leadingAnchor.constraint(equalTo: self.view.leadingAnchor).isActive = true
//            text.trailingAnchor.constraint(equalTo: self.view.trailingAnchor).isActive = true
//            text.setContentHuggingPriority(.fittingSizeCompression, for: .vertical)
//            text.addConstraint(text.heightAnchor.constraint(equalToConstant: 100))
        }
    }
    
    func handle(line: Data, context: CedContext) {
          DispatchQueue.main.sync {
            if let message = try? RpcRequest(with: line) {
                switch message.method {
                case "info":
                    handle_info(params: message.params, context: context)
                case "view":
                    handle_view(params: message.params, context: context)
                case let method:
                    print("unknown notification method: \(method)")
                }
            } else if let message = try? RpcResponse(with: line) {
                if !message.isError() {
                    if let request = self.runningRequests[message.id] {
                        self.handle_rpc_response(request: request, response: message, context: context)
                        self.runningRequests.removeValue(forKey: message.id)
                    } else {
                        print("unexpected response: \(message)")
                    }
                } else {
                    print("error: \(message.error!)")
                }
            } else {
                print("invalid payload: \(String(data: line, encoding: .utf8) ?? "<empty>")")
            }
        }
    }
    
    func handle_info(params: Any?, context: CedContext) {
        let params = params as! [String: Any]
        
        context.session = params["session"] as? String
        self.setWindowTitle(context)
    }
    
    func handle_view(params: Any?, context: CedContext) {
        context.view = params as! [Any]
        self.viewItems.items = context.view as! [[String: Any]]
    }
    
    func handle_rpc_response(request: RpcRequest, response: RpcResponse, context: CedContext) {
        switch request.method {
        case "edit", "menu-select":
            break
        case let method:
            print("unknown response method: \(method)")
        }
    }
    
}

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
    var buffer: NSTextView
    
    init(buffer: NSTextView) {
        self.buffer = buffer
    }
    
    func setWindowTitle(_ context: CedContext) {
        self.window?.title = "\(context.currentBuffer!) - \(context.session!)"
    }
    
    func handle(line: Data, context: CedContext) {
          DispatchQueue.main.sync {
            if let message = try? RpcRequest(with: line) {
                if message.method == "init" {
                    handle_init(params: message.params, context: context)
                } else if message.method == "buffer-changed" {
                    handle_buffer_changed(params: message.params, context: context)
                } else {
                    print("method: \(message.method)\nparams: \(String(describing: message.params))")
                }
            } else if let message = try? RpcResponse(with: line) {
                if !message.isError() {
                    if let request = self.runningRequests[message.id] {
                        self.handle_rpc_response(request: request, response: message, context: context)
                        self.runningRequests.removeValue(forKey: message.id)
                    } else {
                        print("unsolicited response: \(message)")
                    }
                } else {
                    print("error: \(message.error!)")
                }
            } else {
                print("invalid payload: \(String(data: line, encoding: .utf8) ?? "<empty>")")
            }
        }
    }
    
    func handle_init(params: Any?, context: CedContext) {
        let params = params as! [String: Any]
        let bufferListParams = params["buffer_list"] as! [[String: String]]
        
        context.session = params["session"] as! String
        context.currentBuffer = params["buffer_current"] as! String
        context.bufferList = [:]
        for buffer in bufferListParams {
            context.bufferList[buffer["name"]!] = buffer
        }
        
        self.buffer.string = context.buffer()["content"]!
        self.setWindowTitle(context)
    }
    
    func handle_buffer_changed(params: Any?, context: CedContext) {
        let buffer = params as! [String: String]
        let buffer_name = buffer["name"]!
        context.currentBuffer = buffer_name
        context.bufferList[buffer_name] = buffer
    }
    
    func handle_rpc_response(request: RpcRequest, response: RpcResponse, context: CedContext) {
        if request.method == "buffer-select" || request.method == "edit" {
            self.handle_buffer_select(request: request, response: response, context: context)
        } else {
            print("method: \(request.method)\nparams: \(String(describing: request.params))\nresult: \(String(describing: response.result))")
        }
    }
    
    func handle_buffer_select(request: RpcRequest, response: RpcResponse, context: CedContext) {
        let buffer_name = response.result as! String
        context.currentBuffer = buffer_name
        
        self.buffer.string = context.buffer()["content"]!
        self.setWindowTitle(context)
    }
    
}

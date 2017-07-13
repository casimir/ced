import json


class Request(object):

    def __init__(self, id_, method, params):
        self.id = id_
        self.method = method
        self.params = params

    def to_dict(self):
        message = {
            'jsonrpc': "2.0",
            'id': self.id,
            'method': self.method,
        }
        if self.params is not None:
            message['params'] = self.params,
        return message

    def __str__(self):
        return json.dumps(self.to_dict())


class Response(Request):

    @classmethod
    def parse(cls, raw):
        message = json.loads(raw)
        return cls(**message)

    def __init__(self, **kwargs):
        super(Response, self).__init__(
            id_=kwargs.get('id'), method=kwargs.get('method'), params=kwargs.get('params')
        )
        self.result = kwargs.get('result')
        self.error = kwargs.get('error')

    def is_notification(self):
        return self.id is None

    def is_success(self):
        return not self.is_notification() and self.result is not None

    def to_dict(self):
        message = {
            'jsonrpc': "2.0",
        }
        if self.is_notification():
            message['method'] = self.method
            if self.params is not None:
                message['params'] = self.params
        elif self.is_success():
            message['result'] = self.result
        else:
            message['error'] = self.error
        return message


def print_type(ty):
    print('Name:', ty.GetName())
    print('Type class', ty.GetTypeClass())
    print('Number of template arguments:', ty.GetNumberOfTemplateArguments())
    for i in range(ty.GetNumberOfTemplateArguments()):
        print(' ', i, 'kind:', ty.GetTemplateArgumentKind(i), 'type:', ty.GetTemplateArgumentType())

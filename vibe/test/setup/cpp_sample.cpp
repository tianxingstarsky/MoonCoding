#include <string>
#include <utility>

class Project {
public:
    explicit Project(std::string root);
    const std::string &root() const;

private:
    std::string root_;
};

Project::Project(std::string root)
    : root_(std::move(root))
{
}

const std::string &Project::root() const
{
    return root_;
}

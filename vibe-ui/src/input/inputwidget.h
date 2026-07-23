#pragma once



#include <QStringList>

#include <QWidget>



class QEvent;

class QLabel;

class QPushButton;

class QTextEdit;

class QToolButton;



class InputWidget final : public QWidget

{

    Q_OBJECT



public:

    explicit InputWidget(QWidget *parent = nullptr);



    bool eventFilter(QObject *watched, QEvent *event) override;

    QTextEdit *editor() const { return m_editor; }



public slots:

    void setAgentBusy(bool busy);

    void setBackendReady(bool ready);

    void clearDraft();

    void focusEditor();

    void setContextModel(const QString &modelName);

    void setContextTokens(quint64 tokensIn, quint64 tokensOut);

    void setContextSteps(quint64 steps);

    void setKeyboardButtonChecked(bool checked);



signals:

    void messageSubmitted(const QString &message);

    void interruptRequested();

    void settingsRequested();

    void softKeyboardToggleRequested();



private slots:

    void submit();

    void attachFiles();

    void updateFooter();

    void adjustEditorHeight();



private:

    QTextEdit *m_editor;

    QToolButton *m_attachButton;

    QToolButton *m_keyboardButton;

    QPushButton *m_sendButton;

    QLabel *m_footer;

    QWidget *m_contextBar;

    QPushButton *m_contextModelBtn;

    QLabel *m_contextInfo;

    QStringList m_attachedFiles;

    quint64 m_contextSteps = 0;

    quint64 m_contextTokensIn = 0;

    quint64 m_contextTokensOut = 0;

    bool m_busy = false;

    bool m_ready = false;



    void refreshContextInfo();

};


